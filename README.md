## Basic

This crate provides the function `lease`, which turns references into runtime-checked `LeaseRef<T>` and `LeaseMut<T>`.
These can be used to access that references without having the lifetime around.
This makes sending `&mut T` to callbacks from other languages (like python) much easier.

## Usage

Lease a single `&mut T`:
```rs
let mut value = 5u32;
lease(&mut value, |mut lease| {
    // Do whatever you want with 'lease' here
    if let Err(_) = lease.with(|value| *value = 1) {
        println!("Lease invalid");
    }
});
println!("new value: {value}");
```

Lease multiple `&mut T`:
```rs
let mut value1 = 4u32;
let mut value2 = 2u64;
lease((&mut value1, &mut value2), |(mut lease1, mut lease2)| {
    if let Err(_) = lease1.with(|value| *value = 6) {
        println!("Lease invalid");
    }
    if let Err(_) = lease2.with(|value| *value = 9) {
        println!("Lease invalid");
    }
});
println!("new values: {value1}, {value2}");
```

## Thread-safety

The leases are not thread-safe and therefore don't implement `Send`. Thread-safety may come in a future version
