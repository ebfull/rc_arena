# rc_arena [![Build status](https://api.travis-ci.org/ebfull/rc_arena.svg)](https://travis-ci.org/ebfull/rc_arena) [![Crates.io](https://img.shields.io/crates/v/rc_arena.svg)](https://crates.io/crates/rc_arena) #

### [Documentation](https://ebfull.github.io/rc_arena/)

Alternative to [typed-arena](https://crates.io/crates/typed-arena) that instead returns reference counted pointers
to the underlying objects, avoiding lifetime bounds but incurring a runtime penalty. Useful when you wish to create
a large number of `Rc<T>` objects but want them close together in memory, or wish to avoid expensive allocations.

As with all arenas, it's also useful if you don't care about deallocation until after you're done with the entire
collection of objects.

## How do I use it?

Cargo.toml:

```toml
[dependencies]
rc_arena = "0.1.0"
```

Code:

```rust
extern crate rc_arena;

use rc_arena::Arena;

fn main() {
	let arena = Arena::new();
	
	let foo = arena.alloc([1,2,3,4]);
	let bar = arena.alloc([5,6,7,8]);
	let baz = foo.clone();

	assert_eq!(foo[0], 1);
	assert_eq!(bar[0], 5);
	assert_eq!(baz[0], 1);
}
```

## See also

* [`owning_ref`](https://crates.io/crates/owning_ref)