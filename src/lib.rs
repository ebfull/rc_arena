//! This crate provides a special allocator of `Rc`-like objects which reside close
//! together in memory. This is useful when you want to create a large number of `Rc`
//! objects, but you want them close together in memory or at least know how many you
//! want to create in advance. Only once all of the reference counted smart pointers
//! and the arena are dropped, are the underlying objects dropped.
//!
//! # Example:
//! ```rust
//! use rc_arena::Arena;
//! 
//! let arena = Arena::new();
//! let foo = arena.alloc(1);
//! let bar = arena.alloc(2);
//!
//! drop(arena); // objects can outlive the arena if we want
//! 
//! let baz = foo.clone();
//!
//! assert_eq!(*baz, 1);
//! ```

use std::cell::RefCell;
use std::ops::Deref;
use std::hash::Hash;
use std::hash::Hasher;

/// A reference counted pointer to an object that lives in an arena.
pub struct Rc<T> {
    chunks: std::rc::Rc<RefCell<Vec<Vec<T>>>>,
    // Similar to Rc itself, we choose a weird name here because of a privacy check
    // bug in rustc.
    _ptr: *mut T
}

impl<T> Clone for Rc<T> {
    fn clone(&self) -> Rc<T> {
        Rc {
            chunks: self.chunks.clone(),
            _ptr: self._ptr
        }
    }
}

impl<T> Deref for Rc<T> {
    type Target = T;

    fn deref(&self) -> &T {
        // This is okay because the pointer will never outlive the chunks, and
        // the chunks must still exist as this object contains a reference
        // counted pointer to it.
        unsafe { &*self._ptr }
    }
}

impl<T> std::fmt::Display for Rc<T> where T: std::fmt::Display {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        self.deref().fmt(f)
    }
}

impl<T> std::fmt::Debug for Rc<T> where T: std::fmt::Debug {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        self.deref().fmt(f)
    }
}

impl<T: PartialEq> PartialEq for Rc<T> {
    fn eq(&self, other: &Rc<T>) -> bool {
        PartialEq::eq(self.deref(), other.deref())
    }
}

impl<T: PartialEq> Eq for Rc<T> {}

impl<T: Hash> Hash for Rc<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hash::hash(self.deref(), state)
    }
}

impl<T> std::borrow::Borrow<T> for Rc<T> {
    fn borrow(&self) -> &T {
        self.deref()
    }
}

impl<T> std::fmt::Pointer for Rc<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        std::fmt::Pointer::fmt(&(self.deref() as *const T), f)
    }
}

/// A typed arena that provides reference-counted pointers to its underlying
/// objects.
#[derive(Clone)]
pub struct Arena<T> {
    chunks: std::rc::Rc<RefCell<Vec<Vec<T>>>>
}

impl<T> Arena<T> {
    /// Create a new arena with an unspecified capacity.
    pub fn new() -> Arena<T> {
        Arena::with_capacity(8)
    }

    /// Create a new arena with a known initial capacity.
    pub fn with_capacity(n: usize) -> Arena<T> {
        Arena {
            chunks: std::rc::Rc::new(RefCell::new(vec![Vec::with_capacity(n)]))
        }
    }

    /// Store an object in the arena, returning a reference counted
    /// pointer to it.
    ///
    /// ```rust
    /// use rc_arena::Arena;
    /// 
    /// let arena = Arena::new();
    /// let foo = arena.alloc([0; 256]);
    /// let bar = arena.alloc([1; 256]);
    /// let baz = bar.clone();
    /// 
    /// assert_eq!(foo[0], 0);
    /// assert_eq!(bar[0], 1);
    /// assert_eq!(baz[0], 1);
    /// ```
    pub fn alloc(&self, value: T) -> Rc<T> {
        let mut chunks_borrow = self.chunks.borrow_mut();
        let next_chunk_index = chunks_borrow.len();

        let (last_child_length, last_chunk_capacity) = {
            let last_chunk = &chunks_borrow[next_chunk_index - 1];
            (last_chunk.len(), last_chunk.capacity())
        };

        let (chunk, next_item_index) = if last_child_length < last_chunk_capacity {
            (&mut chunks_borrow[next_chunk_index - 1], last_child_length)
        } else {
            let new_capacity = last_chunk_capacity.checked_mul(2).unwrap();
            chunks_borrow.push(Vec::with_capacity(new_capacity));
            (&mut chunks_borrow[next_chunk_index], 0)
        };
        chunk.push(value);
        let new_item_ref = &mut chunk[next_item_index];

        Rc {
            chunks: self.chunks.clone(),
            _ptr: new_item_ref
        }
    }

    /// Get the number of objects currently placed in the arena.
    pub fn len(&self) -> usize {
        let chunks = self.chunks.borrow();

        chunks.iter().map(|a| a.len()).fold(0, |a, b| a+b)
    }

    /// Iterate over the objects in the arena, accepting a closure which
    /// will be passed a reference to the Rc of the object. This may be
    /// deprecated in favor of a (safe) iterator API in the future.
    ///
    /// This will always iterate in the order that the objects were
    /// allocated.
    ///
    /// ```rust
    /// use rc_arena::Arena;
    /// 
    /// let arena = Arena::new();
    /// arena.alloc("Hello,");
    /// arena.alloc(" ");
    /// arena.alloc("world!\n");
    /// 
    /// arena.each(|obj| {
    ///     print!("{}", obj);
    /// });
    /// ```
    pub fn each<F: for<'a> FnMut(&'a Rc<T>)>(&self, mut f: F) {
        use std::ptr;

        let chunks = self.chunks.borrow();

        let mut rc = Rc {
            chunks: self.chunks.clone(),
            _ptr: ptr::null_mut()
        };

        for val in chunks.iter().flat_map(|chunk| chunk.iter()) {
            rc._ptr = val as *const T as *mut T;

            f(&rc);
        }
    }
}

#[test]
fn basic_usecase() {
    let arena: Arena<usize> = Arena::new();
    let test1 = arena.alloc(1);
    let test2 = arena.alloc(2);

    let test = test1.clone();
    assert_eq!(*test, 1);

    drop(test1);

    assert_eq!(*test, 1);

    drop(test);

    assert_eq!(arena.len(), 2);

    drop(arena);

    assert_eq!(*test2, 2);

    drop(test2);
}

#[test]
fn drops_at_once() {
    use std::sync::mpsc::{TryRecvError, Sender, channel};

    struct Foo {
        tx: Sender<()>
    }

    impl Drop for Foo {
        fn drop(&mut self) {
            self.tx.send(()).unwrap();
        }
    }

    let (tx, rx) = channel();

    let arena = Arena::new();
    let test1 = arena.alloc(Foo {
        tx: tx.clone()
    });
    let test2 = arena.alloc(Foo {
        tx: tx.clone()
    });

    let test3 = test1.clone();

    drop(tx);
    assert_eq!(rx.try_recv().err().unwrap(), TryRecvError::Empty);
    drop(arena);
    assert_eq!(rx.try_recv().err().unwrap(), TryRecvError::Empty);
    drop(test1);
    assert_eq!(rx.try_recv().err().unwrap(), TryRecvError::Empty);
    drop(test2);
    assert_eq!(rx.try_recv().err().unwrap(), TryRecvError::Empty);
    drop(test3);


    assert_eq!(rx.recv().unwrap(), ());
    assert_eq!(rx.recv().unwrap(), ());
    assert_eq!(rx.try_recv().err().unwrap(), TryRecvError::Disconnected);
}


#[test]
fn iterates() {
    let arena: Arena<usize> = Arena::with_capacity(2);
    arena.alloc(1);
    arena.alloc(2);
    arena.alloc(3);
    arena.alloc(4);
    arena.alloc(5);

    let mut expected = 1;

    arena.each(|t| {
        assert_eq!(**t, expected);
        expected += 1;
    });

    assert_eq!(expected, 6);
    assert_eq!(arena.len(), 5);
}


#[test]
fn formatting() {
    let arena: Arena<usize> = Arena::with_capacity(5);
    let test1 = arena.alloc(1);
    let test2 = arena.alloc(2);
    arena.alloc(3);
    arena.alloc(4);
    arena.alloc(5);

    assert_eq!("1", &*format!("{}", test1));
    assert_eq!("2", &*format!("{}", test2));
    assert_eq!("1", &*format!("{:?}", test1));
}
