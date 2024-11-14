use std::cell::RefCell;

type Rng = rand::rngs::ThreadRng;

thread_local! {
    static RNG: RefCell<Option<Rng>> = RefCell::new(None);
}

pub fn init_rng() {
    RNG.with(|rng| {
        let mut rng = rng.borrow_mut();
        *rng = Some(rand::thread_rng());
    });
}

pub fn with_rng<F, T>(func: F) -> T
where
    F: FnOnce(&mut Rng) -> T,
{
    let mut ret: Option<T> = None;
    RNG.with(|rng| {
        let mut rng = rng.borrow_mut();
        let rng = rng.as_mut().expect("rng should have been initialized");
        ret = Some(func(rng));
    });
    ret.expect("should have been initialized with closure return value")
}
