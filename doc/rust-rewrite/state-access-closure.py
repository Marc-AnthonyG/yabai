import re, sys, pathlib, collections

repository = pathlib.Path(sys.argv[1])
sources = sorted(
    path for path in (repository / "src").rglob("*")
    if path.suffix in (".c", ".h", ".m") and "osax" not in path.parts and path.name != "manifest.m"
)

header_pattern = re.compile(r"^(?:[A-Za-z_].*\)|[-+]\s*\(.*)\s*$")
name_in_header = re.compile(r"([A-Za-z_][A-Za-z0-9_]*)\s*\(")
macro_headers = {
    "EVENT_HANDLER": "EVENT_HANDLER_{0}",
}
global_pattern = re.compile(r"\bg_[a-z][a-z0-9_]*\b")
call_pattern = re.compile(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\(")
manager_parameter = re.compile(r"struct\s+(window_manager|space_manager|display_manager|process_manager|mouse_state|event_loop)\s*\*")

functions = {}
for path in sources:
    lines = path.read_text(errors="replace").splitlines()
    index = 0
    while index < len(lines) - 1:
        line = lines[index]
        if header_pattern.match(line) and not line.startswith(("#", "//", "typedef", "}")) and lines[index + 1].rstrip() == "{":
            header = line
            back = index - 1
            while back >= 0 and lines[back] and not lines[back].startswith(("#", "}", "{", "//")) and not lines[back].rstrip().endswith((";", ")")) and not lines[back].startswith(" "):
                header = lines[back] + " " + header
                back -= 1
            names = name_in_header.findall(header)
            name = None
            if header.lstrip().startswith(("-", "+")):
                selector_parts = re.findall(r"([A-Za-z_][A-Za-z0-9_]*)\s*:", header)
                bare_selector = re.search(r"\)\s*([A-Za-z_][A-Za-z0-9_]*)\s*$", header)
                name = "objc_method_" + ("_".join(selector_parts) if selector_parts else bare_selector.group(1))
                names = []
            table_macro = re.search(r"\bTABLE_(?:HASH|COMPARE)_FUNC\s*\(\s*([a-z_0-9]+)\s*\)", header)
            if table_macro:
                name = table_macro.group(1)
                names = []
            for candidate in names:
                if candidate in macro_headers:
                    argument = re.search(candidate + r"\s*\(\s*([A-Za-z0-9_]+)", header)
                    name = macro_headers[candidate].format(argument.group(1))
                    break
                if candidate not in ("__attribute__", "__attribute", "unused", "MOUSE_HANDLER", "OBSERVER_CALLBACK", "CONNECTION_CALLBACK", "PROCESS_EVENT_HANDLER", "DISPLAY_EVENT_HANDLER"):
                    name = candidate
                    break
            if name is None:
                callback_macro = re.search(r"\b([A-Z_]+)\s*\(\s*([a-z_][a-z0-9_]*)\s*\)", header)
                if callback_macro:
                    name = callback_macro.group(2)
            end = index + 2
            while end < len(lines) and lines[end].rstrip() != "}":
                end += 1
            body = "\n".join(lines[index + 2:end])
            if name:
                functions[name] = {
                    "file": str(path.relative_to(repository)),
                    "line": index + 1,
                    "header": " ".join(header.split()),
                    "body": body,
                }
            index = end
        index += 1

for name, function in functions.items():
    function["explicit_managers"] = sorted(set(manager_parameter.findall(function["header"])))
    function["direct_globals"] = sorted(set(global_pattern.findall(function["body"])))
    function["callees"] = sorted({callee for callee in call_pattern.findall(function["body"]) if callee in functions and callee != name})

closure = {name: set(function["direct_globals"]) for name, function in functions.items()}
changed = True
while changed:
    changed = False
    for name, function in functions.items():
        for callee in function["callees"]:
            before = len(closure[name])
            closure[name] |= closure[callee]
            if len(closure[name]) != before:
                changed = True

callers = collections.defaultdict(set)
for name, function in functions.items():
    for callee in function["callees"]:
        callers[callee].add(name)

output = pathlib.Path(sys.argv[2])
with output.open("w") as handle:
    handle.write("function\tfile\tline\texplicit_manager_parameters\tdirect_globals\ttransitive_globals\tcalled_from_other_files\n")
    for name in sorted(functions, key=lambda key: (functions[key]["file"], functions[key]["line"])):
        function = functions[name]
        foreign = sorted({functions[caller]["file"] for caller in callers[name]} - {function["file"]})
        handle.write("\t".join([
            name, function["file"], str(function["line"]),
            ",".join(function["explicit_managers"]) or "-",
            ",".join(function["direct_globals"]) or "-",
            ",".join(sorted(closure[name])) or "-",
            ",".join(foreign) or "-",
        ]) + "\n")

per_file = collections.Counter(function["file"] for function in functions.values())
print(f"{len(functions)} functions extracted from {len(sources)} files")
for file, count in sorted(per_file.items()):
    print(f"  {count:4d}  {file}")
every_global = collections.Counter()
for name in functions:
    for glob in closure[name]:
        every_global[glob] += 1
print("functions reaching each global transitively:")
for glob, count in every_global.most_common():
    print(f"  {count:4d}  {glob}")
