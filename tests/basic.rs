use rand::{rngs::SmallRng, seq::SliceRandom, SeedableRng};
use raw_btree::{Item, RawBTree};

const SEED: &'static [u8; 32] = b"testseedtestseedtestseedtestseed";

#[test]
pub fn insert() {
	let mut btree: RawBTree<Item<usize, usize>> = RawBTree::new();

	for (key, value) in &ITEMS {
		if let Some(_) = btree.insert(Item::cmp, Item::new(*key, *value)) {
			println!("duplicate: {}", key);
		}

		btree.validate(Item::cmp);
	}

	assert!(btree.len() == 100);
}

#[test]
pub fn remove() {
	let mut btree: RawBTree<Item<usize, usize>> = RawBTree::new();

	let mut items = ITEMS;

	for (key, value) in &items {
		btree.insert(Item::cmp, Item::new(*key, *value));
	}

	assert_eq!(btree.len(), items.len());

	let mut rng = SmallRng::from_seed(*SEED);
	items.shuffle(&mut rng);

	for (i, (key, value)) in items.iter().enumerate() {
		let item = btree.remove(Item::key_cmp, &key);
		assert_eq!(item, Some(Item::new(*key, *value)));
		assert_eq!(btree.len(), 99 - i);
		btree.validate(Item::cmp);
	}

	assert!(btree.is_empty())
}

#[test]
pub fn clone() {
	let mut btree: RawBTree<Item<usize, usize>> = RawBTree::new();
	for (key, value) in &ITEMS {
		btree.insert(Item::cmp, Item::new(*key, *value));
	}
	
	let cloned = btree.clone();
	std::mem::drop(btree);
	cloned.validate(Item::cmp);
}

// #[test]
// pub fn item_addresses() {
// 	let mut btree: RawBTreeMap<usize, usize> = RawBTreeMap::new();

// 	for (key, value) in &ITEMS {
// 		btree.insert(usize::cmp, *key, *value);
// 	}

// 	for (key, _) in &ITEMS {
// 		let addr = btree.address_of(usize::cmp, key).ok().unwrap();

// 		match btree.previous_item_address(addr) {
// 			Some(before_addr) => {
// 				assert!(before_addr != addr);
// 				let addr_again = btree.next_item_address(before_addr).unwrap();
// 				assert_eq!(addr_again, addr)
// 			}
// 			None => (),
// 		}

// 		match btree.next_item_address(addr) {
// 			Some(after_addr) => {
// 				assert!(after_addr != addr);
// 				let addr_again = btree.previous_item_address(after_addr).unwrap();
// 				assert_eq!(addr_again, addr)
// 			}
// 			None => (),
// 		}
// 	}
// }

// #[test]
// pub fn valid_addresses() {
// 	let mut btree: RawBTreeMap<usize, usize> = RawBTreeMap::new();

// 	for (key, value) in &ITEMS {
// 		btree.insert(*key, *value);
// 	}

// 	for (key, _) in &ITEMS {
// 		let addr = btree.address_of(key).ok().unwrap();

// 		match btree.previous_front_address(addr) {
// 			Some(before_addr) => {
// 				assert!(before_addr != addr);
// 				let addr_again = btree.next_back_address(before_addr).unwrap();
// 				assert_eq!(addr_again, addr)
// 			},
// 			None => ()
// 		}

// 		let after_addr = btree.next_back_address(addr).unwrap(); // there is always a valid address after an item address.
// 		assert!(after_addr != addr);
// 		let addr_again = btree.previous_front_address(after_addr).unwrap();
// 		assert_eq!(addr_again, addr)
// 	}
// }

// #[test]
// pub fn insert_addresses() {
// 	let mut btree: RawBTreeMap<usize, usize> = RawBTreeMap::new();

// 	for (key, value) in &ITEMS {
// 		let addr = btree.address_of(usize::cmp, key).err().unwrap();
// 		let new_addr = btree.insert_exactly_at(addr, Item::new(*key, *value), None);
// 		assert_eq!(btree.item(new_addr).unwrap().value(), value);
// 	}
// }

// #[test]
// pub fn remove_addresses() {
// 	let items = ITEMS;

// 	for k in 1..items.len() {
// 		let mut btree: RawBTreeMap<usize, usize> = RawBTreeMap::new();

// 		for (key, value) in &items {
// 			btree.insert(usize::cmp, *key, *value);
// 			if btree.len() == k {
// 				break;
// 			}
// 		}

// 		for (key, value) in &items {
// 			match btree.address_of(usize::cmp, key) {
// 				Ok(addr) => {
// 					let (_, addr) = btree.remove_at(addr).unwrap();
// 					btree.insert_at(addr, Item::new(*key, *value));
// 					btree.validate();
// 				}
// 				Err(_) => break,
// 			}
// 		}
// 	}
// }

const ITEMS: [(usize, usize); 100] = [
	(4223, 5948),
	(8175, 4629),
	(1411, 7458),
	(9208, 4040),
	(1246, 2287),
	(6568, 7583),
	(5426, 491),
	(7850, 8789),
	(2034, 9388),
	(1408, 7331),
	(7346, 5820),
	(9712, 4253),
	(5430, 7253),
	(1662, 5278),
	(9322, 777),
	(9256, 8116),
	(7971, 8071),
	(648, 3082),
	(7510, 2207),
	(8394, 7839),
	(57, 8834),
	(7770, 5437),
	(6388, 6755),
	(9177, 9904),
	(6487, 5143),
	(2231, 688),
	(7389, 4472),
	(577, 1930),
	(9130, 3222),
	(2230, 8268),
	(1211, 2354),
	(9237, 3643),
	(2912, 8471),
	(8783, 4977),
	(4325, 9566),
	(9355, 528),
	(9814, 9342),
	(1641, 6027),
	(3009, 8304),
	(4199, 2688),
	(7011, 9579),
	(8391, 8562),
	(1097, 5448),
	(1224, 5844),
	(5309, 2846),
	(7493, 8845),
	(3682, 48),
	(9165, 2755),
	(9959, 7420),
	(8158, 2616),
	(3210, 7795),
	(4418, 7790),
	(5592, 4184),
	(4111, 885),
	(742, 952),
	(2486, 6088),
	(6797, 271),
	(8829, 3005),
	(6444, 5818),
	(6566, 8783),
	(913, 2886),
	(2325, 1260),
	(4382, 3045),
	(5451, 1473),
	(9376, 8133),
	(9036, 4924),
	(5202, 7364),
	(9190, 5619),
	(8190, 2892),
	(9493, 500),
	(3043, 8315),
	(9220, 6396),
	(6400, 5692),
	(2709, 8547),
	(1218, 7403),
	(581, 117),
	(2577, 9373),
	(9349, 3186),
	(9021, 4874),
	(4207, 1781),
	(5201, 5305),
	(7889, 1996),
	(6327, 6377),
	(8120, 2338),
	(8213, 9072),
	(865, 6524),
	(5858, 5331),
	(1904, 3594),
	(9950, 8859),
	(518, 6551),
	(2674, 7081),
	(9848, 618),
	(5120, 5595),
	(259, 9662),
	(3077, 863),
	(4519, 7217),
	(3931, 6743),
	(2575, 6810),
	(1553, 5964),
	(4493, 3677),
];
