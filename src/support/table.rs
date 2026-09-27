pub struct Table<K, V> {
    count: i32,
    capacity: i32,
    max_load: f32,
    hash: fn(&K) -> u64,
    buckets: Vec<Vec<(K, V)>>,
}

impl<K: PartialEq, V> Table<K, V> {
    pub fn new(capacity: i32, hash: fn(&K) -> u64) -> Table<K, V> {
        let capacity = capacity.max(1);
        let mut buckets = Vec::with_capacity(capacity as usize);
        for _ in 0..capacity {
            buckets.push(Vec::new());
        }

        Table {
            count: 0,
            capacity,
            max_load: 0.75f32,
            hash,
            buckets,
        }
    }

    fn bucket_index(&self, key: &K) -> usize {
        ((self.hash)(key) % self.capacity as u64) as usize
    }

    fn rehash(&mut self) {
        let old_buckets = std::mem::take(&mut self.buckets);

        self.count = 0;
        self.capacity = 2 * self.capacity;
        self.buckets = Vec::with_capacity(self.capacity as usize);
        for _ in 0..self.capacity {
            self.buckets.push(Vec::new());
        }

        for old_bucket in old_buckets {
            for (key, value) in old_bucket {
                let bucket_index = self.bucket_index(&key);
                self.buckets[bucket_index].push((key, value));
                self.count += 1;
            }
        }
    }

    pub fn add(&mut self, key: K, value: V) {
        let bucket_index = self.bucket_index(&key);
        let occupied = self.buckets[bucket_index]
            .iter()
            .any(|(bucket_key, _)| *bucket_key == key);

        if !occupied {
            self.buckets[bucket_index].push((key, value));
            self.count += 1;

            let load = (1.0f32 * self.count as f32) / self.capacity as f32;
            if load > self.max_load {
                self.rehash();
            }
        }
    }

    pub fn find(&self, key: &K) -> Option<&V> {
        let bucket_index = self.bucket_index(key);
        self.buckets[bucket_index]
            .iter()
            .find(|(bucket_key, _)| bucket_key == key)
            .map(|(_, value)| value)
    }

    pub fn find_mut(&mut self, key: &K) -> Option<&mut V> {
        let bucket_index = self.bucket_index(key);
        self.buckets[bucket_index]
            .iter_mut()
            .find(|(bucket_key, _)| bucket_key == key)
            .map(|(_, value)| value)
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        let bucket_index = self.bucket_index(key);
        let position = self.buckets[bucket_index]
            .iter()
            .position(|(bucket_key, _)| bucket_key == key);

        match position {
            Some(position) => {
                let (_, value) = self.buckets[bucket_index].remove(position);
                self.count -= 1;
                Some(value)
            }
            None => None,
        }
    }

    pub fn len(&self) -> i32 {
        self.count
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.buckets
            .iter()
            .flat_map(|bucket| bucket.iter().map(|(key, value)| (key, value)))
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.buckets
            .iter()
            .flat_map(|bucket| bucket.iter().map(|(_, value)| value))
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.buckets
            .iter_mut()
            .flat_map(|bucket| bucket.iter_mut().map(|(_, value)| value))
    }
}

impl<K: PartialEq + Clone, V> Table<K, V> {
    pub fn keys_in_bucket_order(&self) -> Vec<K> {
        self.buckets
            .iter()
            .flat_map(|bucket| bucket.iter().map(|(key, _)| key.clone()))
            .collect()
    }
}
