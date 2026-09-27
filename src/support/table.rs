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

#[cfg(test)]
mod tests {
    use super::Table;

    fn hash_a_window_id_as_the_window_manager_does(key: &u32) -> u64 {
        *key as u64
    }

    fn hash_a_space_id_as_the_space_manager_does(key: &u64) -> u64 {
        *key
    }

    fn add_each_key_with_its_value_plus_one_hundred(table: &mut Table<u32, u32>, keys: &[u32]) {
        for key in keys {
            table.add(*key, key + 100);
        }
    }

    #[test]
    fn a_table_created_with_capacity_zero_accepts_an_add_and_finds_the_value_again() {
        let mut table: Table<u32, &str> =
            Table::new(0, hash_a_window_id_as_the_window_manager_does);

        table.add(42, "value");

        assert_eq!(table.find(&42), Some(&"value"));
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn a_table_created_with_capacity_zero_keeps_every_key_as_it_grows() {
        let mut table: Table<u32, u32> = Table::new(0, hash_a_window_id_as_the_window_manager_does);

        add_each_key_with_its_value_plus_one_hundred(&mut table, &[5, 1, 9, 2, 7]);

        assert_eq!(table.len(), 5);
        for key in [5, 1, 9, 2, 7] {
            assert_eq!(table.find(&key), Some(&(key + 100)));
        }
    }

    #[test]
    fn add_does_not_overwrite_the_value_of_a_key_already_present() {
        let mut table: Table<u32, &str> =
            Table::new(4, hash_a_window_id_as_the_window_manager_does);

        table.add(11, "first");
        table.add(11, "second");

        assert_eq!(table.find(&11), Some(&"first"));
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn remove_returns_the_value_and_a_later_find_misses() {
        let mut table: Table<u32, &str> =
            Table::new(4, hash_a_window_id_as_the_window_manager_does);
        table.add(3, "three");
        table.add(7, "seven");

        assert_eq!(table.remove(&3), Some("three"));

        assert!(table.find(&3).is_none());
        assert_eq!(table.find(&7), Some(&"seven"));
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn remove_of_a_missing_key_returns_nothing_and_leaves_the_count_alone() {
        let mut table: Table<u32, &str> =
            Table::new(4, hash_a_window_id_as_the_window_manager_does);
        table.add(3, "three");

        assert_eq!(table.remove(&4), None);

        assert_eq!(table.len(), 1);
    }

    #[test]
    fn find_mut_changes_the_stored_value_in_place() {
        let mut table: Table<u32, u32> = Table::new(4, hash_a_window_id_as_the_window_manager_does);
        table.add(3, 1);

        if let Some(value) = table.find_mut(&3) {
            *value = 2;
        }

        assert_eq!(table.find(&3), Some(&2));
    }

    #[test]
    fn iteration_follows_the_c_bucket_order_while_keys_share_one_bucket() {
        let mut table: Table<u32, u32> = Table::new(4, hash_a_window_id_as_the_window_manager_does);

        add_each_key_with_its_value_plus_one_hundred(&mut table, &[7, 3, 11]);

        assert_eq!(table.keys_in_bucket_order(), vec![7, 3, 11]);
    }

    #[test]
    fn iteration_follows_the_c_bucket_order_after_the_first_growth() {
        let mut table: Table<u32, u32> = Table::new(4, hash_a_window_id_as_the_window_manager_does);

        add_each_key_with_its_value_plus_one_hundred(&mut table, &[7, 3, 11, 2]);
        assert_eq!(table.keys_in_bucket_order(), vec![2, 3, 11, 7]);

        add_each_key_with_its_value_plus_one_hundred(&mut table, &[15, 19]);
        assert_eq!(table.keys_in_bucket_order(), vec![2, 3, 11, 19, 7, 15]);
    }

    #[test]
    fn iteration_follows_the_c_bucket_order_after_the_second_growth() {
        let mut table: Table<u32, u32> = Table::new(4, hash_a_window_id_as_the_window_manager_does);

        add_each_key_with_its_value_plus_one_hundred(&mut table, &[7, 3, 11, 2, 15, 19, 8]);

        assert_eq!(table.keys_in_bucket_order(), vec![2, 3, 19, 7, 8, 11, 15]);
        assert_eq!(table.len(), 7);
    }

    #[test]
    fn a_key_added_again_after_its_removal_goes_to_the_end_of_its_bucket_as_in_c() {
        let mut table: Table<u32, u32> = Table::new(4, hash_a_window_id_as_the_window_manager_does);
        add_each_key_with_its_value_plus_one_hundred(&mut table, &[7, 3, 11, 2, 15, 19, 8]);

        table.remove(&3);
        assert_eq!(table.keys_in_bucket_order(), vec![2, 19, 7, 8, 11, 15]);

        table.add(3, 999);
        assert_eq!(table.keys_in_bucket_order(), vec![2, 19, 3, 7, 8, 11, 15]);
    }

    #[test]
    fn iter_and_values_walk_the_same_bucket_order_as_the_keys() {
        let mut table: Table<u32, u32> = Table::new(4, hash_a_window_id_as_the_window_manager_does);
        add_each_key_with_its_value_plus_one_hundred(&mut table, &[7, 3, 11, 2, 15, 19, 8]);

        let pairs: Vec<(u32, u32)> = table.iter().map(|(key, value)| (*key, *value)).collect();
        let values: Vec<u32> = table.values().copied().collect();

        assert_eq!(
            pairs,
            vec![
                (2, 102),
                (3, 103),
                (19, 119),
                (7, 107),
                (8, 108),
                (11, 111),
                (15, 115)
            ]
        );
        assert_eq!(values, vec![102, 103, 119, 107, 108, 111, 115]);
    }

    #[test]
    fn iteration_of_the_view_table_follows_the_c_bucket_order_before_and_after_it_grows() {
        let mut table: Table<u64, ()> = Table::new(23, hash_a_space_id_as_the_space_manager_does);
        for space_id in [
            1,
            3,
            5,
            24,
            47,
            2,
            0x1_0000_0001,
            26,
            70,
            4,
            6,
            7,
            8,
            9,
            10,
            11,
            12,
        ] {
            table.add(space_id, ());
        }
        assert_eq!(
            table.keys_in_bucket_order(),
            vec![
                1, 24, 47, 70, 2, 3, 26, 4, 5, 6, 7, 8, 9, 10, 11, 12, 4294967297
            ]
        );

        table.add(13, ());

        assert_eq!(
            table.keys_in_bucket_order(),
            vec![
                1, 47, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 4294967297, 13, 24, 70, 26
            ]
        );
    }
}
