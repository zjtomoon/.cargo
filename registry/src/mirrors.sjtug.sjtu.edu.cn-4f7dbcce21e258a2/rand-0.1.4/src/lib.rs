// Copyright 2013-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Utilities for random number generation
//!
//! The key functions are `random()` and `Rng::gen()`. These are polymorphic
//! and so can be used to generate any type that implements `Rand`. Type inference
//! means that often a simple call to `rand::random()` or `rng.gen()` will
//! suffice, but sometimes an annotation is required, e.g. `rand::random::<f64>()`.
//!
//! See the `distributions` submodule for sampling random numbers from
//! distributions like normal and exponential.
//!
//! # Thread-local RNG
//!
//! There is built-in support for a RNG associated with each thread stored
//! in thread-local storage. This RNG can be accessed via `thread_rng`, or
//! used implicitly via `random`. This RNG is normally randomly seeded
//! from an operating-system source of randomness, e.g. `/dev/urandom` on
//! Unix systems, and will automatically reseed itself from this source
//! after generating 32 KiB of random data.
//!
//! # Cryptographic security
//!
//! An application that requires an entropy source for cryptographic purposes
//! must use `OsRng`, which reads randomness from the source that the operating
//! system provides (e.g. `/dev/urandom` on Unixes or `CryptGenRandom()` on Windows).
//! The other random number generators provided by this module are not suitable
//! for such purposes.
//!
//! *Note*: many Unix systems provide `/dev/random` as well as `/dev/urandom`.
//! This module uses `/dev/urandom` for the following reasons:
//!
//! -   On Linux, `/dev/random` may block if entropy pool is empty;
//!     `/dev/urandom` will not block.  This does not mean that `/dev/random`
//!     provides better output than `/dev/urandom`; the kernel internally runs a
//!     cryptographically secure pseudorandom number generator (CSPRNG) based on
//!     entropy pool for random number generation, so the "quality" of
//!     `/dev/random` is not better than `/dev/urandom` in most cases.  However,
//!     this means that `/dev/urandom` can yield somewhat predictable randomness
//!     if the entropy pool is very small, such as immediately after first
//!     booting.  Linux 3.17 added the `getrandom(2)` system call which solves
//!     the issue: it blocks if entropy pool is not initialized yet, but it does
//!     not block once initialized.  `OsRng` tries to use `getrandom(2)` if
//!     available, and use `/dev/urandom` fallback if not.  If an application
//!     does not have `getrandom` and likely to be run soon after first booting,
//!     or on a system with very few entropy sources, one should consider using
//!     `/dev/random` via `ReaderRng`.
//! -   On some systems (e.g. FreeBSD, OpenBSD and Mac OS X) there is no
//!     difference between the two sources. (Also note that, on some systems
//!     e.g.  FreeBSD, both `/dev/random` and `/dev/urandom` may block once if
//!     the CSPRNG has not seeded yet.)
//!
//! # Examples
//!
//! ```rust
//! use rand::Rng;
//!
//! let mut rng = rand::thread_rng();
//! if rng.gen() { // random bool
//!     println!("i32: {}, u32: {}", rng.gen::<i32>(), rng.gen::<u32>())
//! }
//! ```
//!
//! ```rust
//! let tuple = rand::random::<(f64, char)>();
//! println!("{:?}", tuple)
//! ```
//!
//! ## Monte Carlo estimation of ??
//!
//! For this example, imagine we have a square with sides of length 2 and a unit
//! circle, both centered at the origin. Since the area of a unit circle is ??,
//! we have:
//!
//! ```text
//!     (area of unit circle) / (area of square) = ?? / 4
//! ```
//!
//! So if we sample many points randomly from the square, roughly ?? / 4 of them
//! should be inside the circle.
//!
//! We can use the above fact to estimate the value of ??: pick many points in
//! the square at random, calculate the fraction that fall within the circle,
//! and multiply this fraction by 4.
//!
//! ```
//! use rand::distributions::{IndependentSample, Range};
//!
//! fn main() {
//!    let between = Range::new(-1f64, 1.);
//!    let mut rng = rand::thread_rng();
//!
//!    let total = 1_000_000;
//!    let mut in_circle = 0;
//!
//!    for _ in 0..total {
//!        let a = between.ind_sample(&mut rng);
//!        let b = between.ind_sample(&mut rng);
//!        if a*a + b*b <= 1. {
//!            in_circle += 1;
//!        }
//!    }
//!
//!    // prints something close to 3.14159...
//!    println!("{}", 4. * (in_circle as f64) / (total as f64));
//! }
//! ```
//!
//! ## Monty Hall Problem
//!
//! This is a simulation of the [Monty Hall Problem][]:
//!
//! > Suppose you're on a game show, and you're given the choice of three doors:
//! > Behind one door is a car; behind the others, goats. You pick a door, say No. 1,
//! > and the host, who knows what's behind the doors, opens another door, say No. 3,
//! > which has a goat. He then says to you, "Do you want to pick door No. 2?"
//! > Is it to your advantage to switch your choice?
//!
//! The rather unintuitive answer is that you will have a 2/3 chance of winning
//! if you switch and a 1/3 chance of winning if you don't, so it's better to
//! switch.
//!
//! This program will simulate the game show and with large enough simulation
//! steps it will indeed confirm that it is better to switch.
//!
//! [Monty Hall Problem]: http://en.wikipedia.org/wiki/Monty_Hall_problem
//!
//! ```
//! use rand::Rng;
//! use rand::distributions::{IndependentSample, Range};
//!
//! struct SimulationResult {
//!     win: bool,
//!     switch: bool,
//! }
//!
//! // Run a single simulation of the Monty Hall problem.
//! fn simulate<R: Rng>(random_door: &Range<usize>, rng: &mut R) -> SimulationResult {
//!     let car = random_door.ind_sample(rng);
//!
//!     // This is our initial choice
//!     let mut choice = random_door.ind_sample(rng);
//!
//!     // The game host opens a door
//!     let open = game_host_open(car, choice, rng);
//!
//!     // Shall we switch?
//!     let switch = rng.gen();
//!     if switch {
//!         choice = switch_door(choice, open);
//!     }
//!
//!     SimulationResult { win: choice == car, switch: switch }
//! }
//!
//! // Returns the door the game host opens given our choice and knowledge of
//! // where the car is. The game host will never open the door with the car.
//! fn game_host_open<R: Rng>(car: usize, choice: usize, rng: &mut R) -> usize {
//!     let choices = free_doors(&[car, choice]);
//!     rand::sample(rng, choices.into_iter(), 1)[0]
//! }
//!
//! // Returns the door we switch to, given our current choice and
//! // the open door. There will only be one valid door.
//! fn switch_door(choice: usize, open: usize) -> usize {
//!     free_doors(&[choice, open])[0]
//! }
//!
//! fn free_doors(blocked: &[usize]) -> Vec<usize> {
//!     (0..3).filter(|x| !blocked.contains(x)).collect()
//! }
//!
//! fn main() {
//!     // The estimation will be more accurate with more simulations
//!     let num_simulations = 10000;
//!
//!     let mut rng = rand::thread_rng();
//!     let random_door = Range::new(0, 3);
//!
//!     let (mut switch_wins, mut switch_losses) = (0, 0);
//!     let (mut keep_wins, mut keep_losses) = (0, 0);
//!
//!     println!("Running {} simulations...", num_simulations);
//!     for _ in 0..num_simulations {
//!         let result = simulate(&random_door, &mut rng);
//!
//!         match (result.win, result.switch) {
//!             (true, true) => switch_wins += 1,
//!             (true, false) => keep_wins += 1,
//!             (false, true) => switch_losses += 1,
//!             (false, false) => keep_losses += 1,
//!         }
//!     }
//!
//!     let total_switches = switch_wins + switch_losses;
//!     let total_keeps = keep_wins + keep_losses;
//!
//!     println!("Switched door {} times with {} wins and {} losses",
//!              total_switches, switch_wins, switch_losses);
//!
//!     println!("Kept our choice {} times with {} wins and {} losses",
//!              total_keeps, keep_wins, keep_losses);
//!
//!     // With a large number of simulations, the values should converge to
//!     // 0.667 and 0.333 respectively.
//!     println!("Estimated chance to win if we switch: {}",
//!              switch_wins as f32 / total_switches as f32);
//!     println!("Estimated chance to win if we don't: {}",
//!              keep_wins as f32 / total_keeps as f32);
//! }
//! ```

#![doc(html_logo_url = "http://www.rust-lang.org/logos/rust-logo-128x128-blk.png",
       html_favicon_url = "http://www.rust-lang.org/favicon.ico",
       html_root_url = "http://doc.rust-lang.org/rand/")]
#![feature(core, os, old_path, old_io)]

#![cfg_attr(test, feature(test))]

extern crate core;
#[cfg(test)] #[macro_use] extern crate log;

use std::cell::RefCell;
use std::marker;
use std::mem;
use std::old_io::IoResult;
use std::rc::Rc;
use std::num::wrapping::Wrapping as w;

pub use os::OsRng;

pub use isaac::{IsaacRng, Isaac64Rng};
pub use chacha::ChaChaRng;

#[cfg(target_pointer_width = "32")]
use IsaacRng as IsaacWordRng;
#[cfg(target_pointer_width = "64")]
use Isaac64Rng as IsaacWordRng;

use distributions::{Range, IndependentSample};
use distributions::range::SampleRange;

pub mod distributions;
pub mod isaac;
pub mod chacha;
pub mod reseeding;
mod rand_impls;
pub mod os;
pub mod reader;

#[allow(bad_style)]
type w64 = w<u64>;
#[allow(bad_style)]
type w32 = w<u32>;

/// A type that can be randomly generated using an `Rng`.
pub trait Rand : Sized {
    /// Generates a random instance of this type using the specified source of
    /// randomness.
    fn rand<R: Rng>(rng: &mut R) -> Self;
}

/// A random number generator.
pub trait Rng : Sized {
    /// Return the next random u32.
    ///
    /// This rarely needs to be called directly, prefer `r.gen()` to
    /// `r.next_u32()`.
    // FIXME #7771: Should be implemented in terms of next_u64
    fn next_u32(&mut self) -> u32;

    /// Return the next random u64.
    ///
    /// By default this is implemented in terms of `next_u32`. An
    /// implementation of this trait must provide at least one of
    /// these two methods. Similarly to `next_u32`, this rarely needs
    /// to be called directly, prefer `r.gen()` to `r.next_u64()`.
    fn next_u64(&mut self) -> u64 {
        ((self.next_u32() as u64) << 32) | (self.next_u32() as u64)
    }

    /// Return the next random f32 selected from the half-open
    /// interval `[0, 1)`.
    ///
    /// By default this is implemented in terms of `next_u32`, but a
    /// random number generator which can generate numbers satisfying
    /// the requirements directly can overload this for performance.
    /// It is required that the return value lies in `[0, 1)`.
    ///
    /// See `Closed01` for the closed interval `[0,1]`, and
    /// `Open01` for the open interval `(0,1)`.
    fn next_f32(&mut self) -> f32 {
        const MANTISSA_BITS: usize = 24;
        const IGNORED_BITS: usize = 8;
        const SCALE: f32 = (1u64 << MANTISSA_BITS) as f32;

        // using any more than `MANTISSA_BITS` bits will
        // cause (e.g.) 0xffff_ffff to correspond to 1
        // exactly, so we need to drop some (8 for f32, 11
        // for f64) to guarantee the open end.
        (self.next_u32() >> IGNORED_BITS) as f32 / SCALE
    }

    /// Return the next random f64 selected from the half-open
    /// interval `[0, 1)`.
    ///
    /// By default this is implemented in terms of `next_u64`, but a
    /// random number generator which can generate numbers satisfying
    /// the requirements directly can overload this for performance.
    /// It is required that the return value lies in `[0, 1)`.
    ///
    /// See `Closed01` for the closed interval `[0,1]`, and
    /// `Open01` for the open interval `(0,1)`.
    fn next_f64(&mut self) -> f64 {
        const MANTISSA_BITS: usize = 53;
        const IGNORED_BITS: usize = 11;
        const SCALE: f64 = (1u64 << MANTISSA_BITS) as f64;

        (self.next_u64() >> IGNORED_BITS) as f64 / SCALE
    }

    /// Fill `dest` with random data.
    ///
    /// This has a default implementation in terms of `next_u64` and
    /// `next_u32`, but should be overridden by implementations that
    /// offer a more efficient solution than just calling those
    /// methods repeatedly.
    ///
    /// This method does *not* have a requirement to bear any fixed
    /// relationship to the other methods, for example, it does *not*
    /// have to result in the same output as progressively filling
    /// `dest` with `self.gen::<u8>()`, and any such behaviour should
    /// not be relied upon.
    ///
    /// This method should guarantee that `dest` is entirely filled
    /// with new data, and may panic if this is impossible
    /// (e.g. reading past the end of a file that is being used as the
    /// source of randomness).
    ///
    /// # Example
    ///
    /// ```rust
    /// use rand::{thread_rng, Rng};
    ///
    /// let mut v = [0u8; 13579];
    /// thread_rng().fill_bytes(&mut v);
    /// println!("{:?}", v.as_slice());
    /// ```
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        // this could, in theory, be done by transmuting dest to a
        // [u64], but this is (1) likely to be undefined behaviour for
        // LLVM, (2) has to be very careful about alignment concerns,
        // (3) adds more `unsafe` that needs to be checked, (4)
        // probably doesn't give much performance gain if
        // optimisations are on.
        let mut count = 0;
        let mut num = 0;
        for byte in dest.iter_mut() {
            if count == 0 {
                // we could micro-optimise here by generating a u32 if
                // we only need a few more bytes to fill the vector
                // (i.e. at most 4).
                num = self.next_u64();
                count = 8;
            }

            *byte = (num & 0xff) as u8;
            num >>= 8;
            count -= 1;
        }
    }

    /// Return a random value of a `Rand` type.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rand::{thread_rng, Rng};
    ///
    /// let mut rng = thread_rng();
    /// let x: u32 = rng.gen();
    /// println!("{}", x);
    /// println!("{:?}", rng.gen::<(f64, bool)>());
    /// ```
    #[inline(always)]
    fn gen<T: Rand>(&mut self) -> T {
        Rand::rand(self)
    }

    /// Return an iterator that will yield an infinite number of randomly
    /// generated items.
    ///
    /// # Example
    ///
    /// ```
    /// use rand::{thread_rng, Rng};
    ///
    /// let mut rng = thread_rng();
    /// let x = rng.gen_iter::<u32>().take(10).collect::<Vec<u32>>();
    /// println!("{:?}", x);
    /// println!("{:?}", rng.gen_iter::<(f64, bool)>().take(5)
    ///                     .collect::<Vec<(f64, bool)>>());
    /// ```
    fn gen_iter<'a, T: Rand>(&'a mut self) -> Generator<'a, T, Self> {
        Generator { rng: self, _marker: marker::PhantomData }
    }

    /// Generate a random value in the range [`low`, `high`).
    ///
    /// This is a convenience wrapper around
    /// `distributions::Range`. If this function will be called
    /// repeatedly with the same arguments, one should use `Range`, as
    /// that will amortize the computations that allow for perfect
    /// uniformity, as they only happen on initialization.
    ///
    /// # Panics
    ///
    /// Panics if `low >= high`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rand::{thread_rng, Rng};
    ///
    /// let mut rng = thread_rng();
    /// let n: u32 = rng.gen_range(0, 10);
    /// println!("{}", n);
    /// let m: f64 = rng.gen_range(-40.0f64, 1.3e5f64);
    /// println!("{}", m);
    /// ```
    fn gen_range<T: PartialOrd + SampleRange>(&mut self, low: T, high: T) -> T {
        assert!(low < high, "Rng.gen_range called with low >= high");
        Range::new(low, high).ind_sample(self)
    }

    /// Return a bool with a 1 in n chance of true
    ///
    /// # Example
    ///
    /// ```rust
    /// use rand::{thread_rng, Rng};
    ///
    /// let mut rng = thread_rng();
    /// println!("{}", rng.gen_weighted_bool(3));
    /// ```
    fn gen_weighted_bool(&mut self, n: usize) -> bool {
        n <= 1 || self.gen_range(0, n) == 0
    }

    /// Return an iterator of random characters from the set A-Z,a-z,0-9.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rand::{thread_rng, Rng};
    ///
    /// let s: String = thread_rng().gen_ascii_chars().take(10).collect();
    /// println!("{}", s);
    /// ```
    fn gen_ascii_chars<'a>(&'a mut self) -> AsciiGenerator<'a, Self> {
        AsciiGenerator { rng: self }
    }

    /// Return a random element from `values`.
    ///
    /// Return `None` if `values` is empty.
    ///
    /// # Example
    ///
    /// ```
    /// use rand::{thread_rng, Rng};
    ///
    /// let choices = [1, 2, 4, 8, 16, 32];
    /// let mut rng = thread_rng();
    /// println!("{:?}", rng.choose(&choices));
    /// assert_eq!(rng.choose(&choices[..0]), None);
    /// ```
    fn choose<'a, T>(&mut self, values: &'a [T]) -> Option<&'a T> {
        if values.is_empty() {
            None
        } else {
            Some(&values[self.gen_range(0, values.len())])
        }
    }

    /// Shuffle a mutable slice in place.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rand::{thread_rng, Rng};
    ///
    /// let mut rng = thread_rng();
    /// let mut y = [1, 2, 3];
    /// rng.shuffle(&mut y);
    /// println!("{:?}", y.as_slice());
    /// rng.shuffle(&mut y);
    /// println!("{:?}", y.as_slice());
    /// ```
    fn shuffle<T>(&mut self, values: &mut [T]) {
        let mut i = values.len();
        while i >= 2 {
            // invariant: elements with index >= i have been locked in place.
            i -= 1;
            // lock element i in place.
            values.swap(i, self.gen_range(0, i + 1));
        }
    }
}

/// Iterator which will generate a stream of random items.
///
/// This iterator is created via the `gen_iter` method on `Rng`.
pub struct Generator<'a, T, R:'a> {
    rng: &'a mut R,
    _marker: marker::PhantomData<fn() -> T>,
}

impl<'a, T: Rand, R: Rng> Iterator for Generator<'a, T, R> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        Some(self.rng.gen())
    }
}

/// Iterator which will continuously generate random ascii characters.
///
/// This iterator is created via the `gen_ascii_chars` method on `Rng`.
pub struct AsciiGenerator<'a, R:'a> {
    rng: &'a mut R,
}

impl<'a, R: Rng> Iterator for AsciiGenerator<'a, R> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        static GEN_ASCII_STR_CHARSET: &'static [u8] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
              abcdefghijklmnopqrstuvwxyz\
              0123456789";
        Some(*self.rng.choose(GEN_ASCII_STR_CHARSET).unwrap() as char)
    }
}

/// A random number generator that can be explicitly seeded to produce
/// the same stream of randomness multiple times.
pub trait SeedableRng<Seed>: Rng {
    /// Reseed an RNG with the given seed.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rand::{Rng, SeedableRng, StdRng};
    ///
    /// let seed: &[_] = &[1, 2, 3, 4];
    /// let mut rng: StdRng = SeedableRng::from_seed(seed);
    /// println!("{}", rng.gen::<f64>());
    /// rng.reseed(&[5, 6, 7, 8]);
    /// println!("{}", rng.gen::<f64>());
    /// ```
    fn reseed(&mut self, Seed);

    /// Create a new RNG with the given seed.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rand::{Rng, SeedableRng, StdRng};
    ///
    /// let seed: &[_] = &[1, 2, 3, 4];
    /// let mut rng: StdRng = SeedableRng::from_seed(seed);
    /// println!("{}", rng.gen::<f64>());
    /// ```
    fn from_seed(seed: Seed) -> Self;
}

/// An Xorshift[1] random number
/// generator.
///
/// The Xorshift algorithm is not suitable for cryptographic purposes
/// but is very fast. If you do not know for sure that it fits your
/// requirements, use a more secure one such as `IsaacRng` or `OsRng`.
///
/// [1]: Marsaglia, George (July 2003). ["Xorshift
/// RNGs"](http://www.jstatsoft.org/v08/i14/paper). *Journal of
/// Statistical Software*. Vol. 8 (Issue 14).
#[allow(missing_copy_implementations)]
#[derive(Clone)]
pub struct XorShiftRng {
    x: w32,
    y: w32,
    z: w32,
    w: w32,
}

impl XorShiftRng {
    /// Creates a new XorShiftRng instance which is not seeded.
    ///
    /// The initial values of this RNG are constants, so all generators created
    /// by this function will yield the same stream of random numbers. It is
    /// highly recommended that this is created through `SeedableRng` instead of
    /// this function
    pub fn new_unseeded() -> XorShiftRng {
        XorShiftRng {
            x: w(0x193a6754),
            y: w(0xa8a7d469),
            z: w(0x97830e05),
            w: w(0x113ba7bb),
        }
    }
}

impl Rng for XorShiftRng {
    #[inline]
    fn next_u32(&mut self) -> u32 {
        let x = self.x;
        let t = x ^ (x << 11);
        self.x = self.y;
        self.y = self.z;
        self.z = self.w;
        let w_ = self.w;
        self.w = w_ ^ (w_ >> 19) ^ (t ^ (t >> 8));
        self.w.0
    }
}

impl SeedableRng<[u32; 4]> for XorShiftRng {
    /// Reseed an XorShiftRng. This will panic if `seed` is entirely 0.
    fn reseed(&mut self, seed: [u32; 4]) {
        assert!(!seed.iter().all(|&x| x == 0),
                "XorShiftRng.reseed called with an all zero seed.");

        self.x = w(seed[0]);
        self.y = w(seed[1]);
        self.z = w(seed[2]);
        self.w = w(seed[3]);
    }

    /// Create a new XorShiftRng. This will panic if `seed` is entirely 0.
    fn from_seed(seed: [u32; 4]) -> XorShiftRng {
        assert!(!seed.iter().all(|&x| x == 0),
                "XorShiftRng::from_seed called with an all zero seed.");

        XorShiftRng {
            x: w(seed[0]),
            y: w(seed[1]),
            z: w(seed[2]),
            w: w(seed[3]),
        }
    }
}

impl Rand for XorShiftRng {
    fn rand<R: Rng>(rng: &mut R) -> XorShiftRng {
        let mut tuple: (u32, u32, u32, u32) = rng.gen();
        while tuple == (0, 0, 0, 0) {
            tuple = rng.gen();
        }
        let (x, y, z, w_) = tuple;
        XorShiftRng { x: w(x), y: w(y), z: w(z), w: w(w_) }
    }
}

/// A wrapper for generating floating point numbers uniformly in the
/// open interval `(0,1)` (not including either endpoint).
///
/// Use `Closed01` for the closed interval `[0,1]`, and the default
/// `Rand` implementation for `f32` and `f64` for the half-open
/// `[0,1)`.
///
/// # Example
/// ```rust
/// use rand::{random, Open01};
///
/// let Open01(val) = random::<Open01<f32>>();
/// println!("f32 from (0,1): {}", val);
/// ```
pub struct Open01<F>(pub F);

/// A wrapper for generating floating point numbers uniformly in the
/// closed interval `[0,1]` (including both endpoints).
///
/// Use `Open01` for the closed interval `(0,1)`, and the default
/// `Rand` implementation of `f32` and `f64` for the half-open
/// `[0,1)`.
///
/// # Example
///
/// ```rust
/// use rand::{random, Closed01};
///
/// let Closed01(val) = random::<Closed01<f32>>();
/// println!("f32 from [0,1]: {}", val);
/// ```
pub struct Closed01<F>(pub F);

/// The standard RNG. This is designed to be efficient on the current
/// platform.
#[derive(Copy, Clone)]
pub struct StdRng {
    rng: IsaacWordRng,
}

impl StdRng {
    /// Create a randomly seeded instance of `StdRng`.
    ///
    /// This is a very expensive operation as it has to read
    /// randomness from the operating system and use this in an
    /// expensive seeding operation. If one is only generating a small
    /// number of random numbers, or doesn't need the utmost speed for
    /// generating each number, `thread_rng` and/or `random` may be more
    /// appropriate.
    ///
    /// Reading the randomness from the OS may fail, and any error is
    /// propagated via the `IoResult` return value.
    pub fn new() -> IoResult<StdRng> {
        OsRng::new().map(|mut r| StdRng { rng: r.gen() })
    }
}

impl Rng for StdRng {
    #[inline]
    fn next_u32(&mut self) -> u32 {
        self.rng.next_u32()
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        self.rng.next_u64()
    }
}

impl<'a> SeedableRng<&'a [usize]> for StdRng {
    fn reseed(&mut self, seed: &'a [usize]) {
        // the internal RNG can just be seeded from the above
        // randomness.
        self.rng.reseed(unsafe {mem::transmute(seed)})
    }

    fn from_seed(seed: &'a [usize]) -> StdRng {
        StdRng { rng: SeedableRng::from_seed(unsafe {mem::transmute(seed)}) }
    }
}

/// Create a weak random number generator with a default algorithm and seed.
///
/// It returns the fastest `Rng` algorithm currently available in Rust without
/// consideration for cryptography or security. If you require a specifically
/// seeded `Rng` for consistency over time you should pick one algorithm and
/// create the `Rng` yourself.
///
/// This will read randomness from the operating system to seed the
/// generator.
pub fn weak_rng() -> XorShiftRng {
    match OsRng::new() {
        Ok(mut r) => r.gen(),
        Err(e) => panic!("weak_rng: failed to create seeded RNG: {:?}", e)
    }
}

/// Controls how the thread-local RNG is reseeded.
struct ThreadRngReseeder;

impl reseeding::Reseeder<StdRng> for ThreadRngReseeder {
    fn reseed(&mut self, rng: &mut StdRng) {
        *rng = match StdRng::new() {
            Ok(r) => r,
            Err(e) => panic!("could not reseed thread_rng: {}", e)
        }
    }
}
static THREAD_RNG_RESEED_THRESHOLD: usize = 32_768;
type ThreadRngInner = reseeding::ReseedingRng<StdRng, ThreadRngReseeder>;

/// The thread-local RNG.
#[derive(Clone)]
pub struct ThreadRng {
    rng: Rc<RefCell<ThreadRngInner>>,
}

/// Retrieve the lazily-initialized thread-local random number
/// generator, seeded by the system. Intended to be used in method
/// chaining style, e.g. `thread_rng().gen::<i32>()`.
///
/// The RNG provided will reseed itself from the operating system
/// after generating a certain amount of randomness.
///
/// The internal RNG used is platform and architecture dependent, even
/// if the operating system random number generator is rigged to give
/// the same sequence always. If absolute consistency is required,
/// explicitly select an RNG, e.g. `IsaacRng` or `Isaac64Rng`.
pub fn thread_rng() -> ThreadRng {
    // used to make space in TLS for a random number generator
    thread_local!(static THREAD_RNG_KEY: Rc<RefCell<ThreadRngInner>> = {
        let r = match StdRng::new() {
            Ok(r) => r,
            Err(e) => panic!("could not initialize thread_rng: {}", e)
        };
        let rng = reseeding::ReseedingRng::new(r,
                                               THREAD_RNG_RESEED_THRESHOLD,
                                               ThreadRngReseeder);
        Rc::new(RefCell::new(rng))
    });

    ThreadRng { rng: THREAD_RNG_KEY.with(|t| t.clone()) }
}

impl Rng for ThreadRng {
    fn next_u32(&mut self) -> u32 {
        self.rng.borrow_mut().next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.rng.borrow_mut().next_u64()
    }

    #[inline]
    fn fill_bytes(&mut self, bytes: &mut [u8]) {
        self.rng.borrow_mut().fill_bytes(bytes)
    }
}

/// Generates a random value using the thread-local random number generator.
///
/// `random()` can generate various types of random things, and so may require
/// type hinting to generate the specific type you want.
///
/// This function uses the thread local random number generator. This means
/// that if you're calling `random()` in a loop, caching the generator can
/// increase performance. An example is shown below.
///
/// # Examples
///
/// ```
/// let x = rand::random();
/// println!("{}", 2u8 * x);
///
/// let y = rand::random::<f64>();
/// println!("{}", y);
///
/// if rand::random() { // generates a boolean
///     println!("Better lucky than good!");
/// }
/// ```
///
/// Caching the thread local random number generator:
///
/// ```
/// use rand::Rng;
///
/// let mut v = vec![1, 2, 3];
///
/// for x in v.iter_mut() {
///     *x = rand::random()
/// }
///
/// // would be faster as
///
/// let mut rng = rand::thread_rng();
///
/// for x in v.iter_mut() {
///     *x = rng.gen();
/// }
/// ```
#[inline]
pub fn random<T: Rand>() -> T {
    thread_rng().gen()
}

/// Randomly sample up to `amount` elements from an iterator.
///
/// # Example
///
/// ```rust
/// use rand::{thread_rng, sample};
///
/// let mut rng = thread_rng();
/// let sample = sample(&mut rng, 1..100, 5);
/// println!("{:?}", sample);
/// ```
pub fn sample<T, I: Iterator<Item=T>, R: Rng>(rng: &mut R,
                                         mut iter: I,
                                         amount: usize) -> Vec<T> {
    let mut reservoir: Vec<T> = iter.by_ref().take(amount).collect();
    for (i, elem) in iter.enumerate() {
        let k = rng.gen_range(0, i + 1 + amount);
        if k < amount {
            reservoir[k] = elem;
        }
    }
    return reservoir;
}

#[cfg(test)]
mod test {
    use super::{Rng, thread_rng, random, SeedableRng, StdRng, sample};
    use std::iter::{order, repeat};

    pub struct MyRng<R> { inner: R }

    impl<R: Rng> Rng for MyRng<R> {
        fn next_u32(&mut self) -> u32 {
            fn next<T: Rng>(t: &mut T) -> u32 {
                t.next_u32()
            }
            next(&mut self.inner)
        }
    }

    pub fn rng() -> MyRng<::ThreadRng> {
        MyRng { inner: ::thread_rng() }
    }

    pub fn weak_rng() -> MyRng<::XorShiftRng> {
        MyRng { inner: ::weak_rng() }
    }

    struct ConstRng { i: u64 }
    impl Rng for ConstRng {
        fn next_u32(&mut self) -> u32 { self.i as u32 }
        fn next_u64(&mut self) -> u64 { self.i }

        // no fill_bytes on purpose
    }

    #[test]
    fn test_fill_bytes_default() {
        let mut r = ConstRng { i: 0x11_22_33_44_55_66_77_88 };

        // check every remainder mod 8, both in small and big vectors.
        let lengths = [0, 1, 2, 3, 4, 5, 6, 7,
                       80, 81, 82, 83, 84, 85, 86, 87];
        for &n in lengths.iter() {
            let mut v = repeat(0u8).take(n).collect::<Vec<_>>();
            r.fill_bytes(v.as_mut_slice());

            // use this to get nicer error messages.
            for (i, &byte) in v.iter().enumerate() {
                if byte == 0 {
                    panic!("byte {} of {} is zero", i, n)
                }
            }
        }
    }

    #[test]
    fn test_gen_range() {
        let mut r = thread_rng();
        for _ in 0..1000 {
            let a = r.gen_range(-3, 42);
            assert!(a >= -3 && a < 42);
            assert_eq!(r.gen_range(0, 1), 0);
            assert_eq!(r.gen_range(-12, -11), -12);
        }

        for _ in 0..1000 {
            let a = r.gen_range(10, 42);
            assert!(a >= 10 && a < 42);
            assert_eq!(r.gen_range(0, 1), 0);
            assert_eq!(r.gen_range(3_000_000, 3_000_001), 3_000_000);
        }

    }

    #[test]
    #[should_fail]
    fn test_gen_range_panic_int() {
        let mut r = thread_rng();
        r.gen_range(5, -2);
    }

    #[test]
    #[should_fail]
    fn test_gen_range_panic_usize() {
        let mut r = thread_rng();
        r.gen_range(5, 2);
    }

    #[test]
    fn test_gen_f64() {
        let mut r = thread_rng();
        let a = r.gen::<f64>();
        let b = r.gen::<f64>();
        debug!("{:?}", (a, b));
    }

    #[test]
    fn test_gen_weighted_bool() {
        let mut r = thread_rng();
        assert_eq!(r.gen_weighted_bool(0), true);
        assert_eq!(r.gen_weighted_bool(1), true);
    }

    #[test]
    fn test_gen_ascii_str() {
        let mut r = thread_rng();
        assert_eq!(r.gen_ascii_chars().take(0).count(), 0);
        assert_eq!(r.gen_ascii_chars().take(10).count(), 10);
        assert_eq!(r.gen_ascii_chars().take(16).count(), 16);
    }

    #[test]
    fn test_gen_vec() {
        let mut r = thread_rng();
        assert_eq!(r.gen_iter::<u8>().take(0).count(), 0);
        assert_eq!(r.gen_iter::<u8>().take(10).count(), 10);
        assert_eq!(r.gen_iter::<f64>().take(16).count(), 16);
    }

    #[test]
    fn test_choose() {
        let mut r = thread_rng();
        assert_eq!(r.choose(&[1, 1, 1]).map(|&x|x), Some(1));

        let v: &[isize] = &[];
        assert_eq!(r.choose(v), None);
    }

    #[test]
    fn test_shuffle() {
        let mut r = thread_rng();
        let empty: &mut [isize] = &mut [];
        r.shuffle(empty);
        let mut one = [1];
        r.shuffle(&mut one);
        let b: &[_] = &[1];
        assert_eq!(one, b);

        let mut two = [1, 2];
        r.shuffle(&mut two);
        assert!(two == [1, 2] || two == [2, 1]);

        let mut x = [1, 1, 1];
        r.shuffle(&mut x);
        let b: &[_] = &[1, 1, 1];
        assert_eq!(x, b);
    }

    #[test]
    fn test_thread_rng() {
        let mut r = thread_rng();
        r.gen::<i32>();
        let mut v = [1, 1, 1];
        r.shuffle(&mut v);
        let b: &[_] = &[1, 1, 1];
        assert_eq!(v, b);
        assert_eq!(r.gen_range(0, 1), 0);
    }

    #[test]
    fn test_random() {
        // not sure how to test this aside from just getting some values
        let _n : usize = random();
        let _f : f32 = random();
        let _o : Option<Option<i8>> = random();
        let _many : ((),
                     (usize,
                      isize,
                      Option<(u32, (bool,))>),
                     (u8, i8, u16, i16, u32, i32, u64, i64),
                     (f32, (f64, (f64,)))) = random();
    }

    #[test]
    fn test_sample() {
        let min_val = 1;
        let max_val = 100;

        let mut r = thread_rng();
        let vals = (min_val..max_val).collect::<Vec<i32>>();
        let small_sample = sample(&mut r, vals.iter(), 5);
        let large_sample = sample(&mut r, vals.iter(), vals.len() + 5);

        assert_eq!(small_sample.len(), 5);
        assert_eq!(large_sample.len(), vals.len());

        assert!(small_sample.iter().all(|e| {
            **e >= min_val && **e <= max_val
        }));
    }

    #[test]
    fn test_std_rng_seeded() {
        let s = thread_rng().gen_iter::<usize>().take(256).collect::<Vec<usize>>();
        let mut ra: StdRng = SeedableRng::from_seed(s.as_slice());
        let mut rb: StdRng = SeedableRng::from_seed(s.as_slice());
        assert!(order::equals(ra.gen_ascii_chars().take(100),
                              rb.gen_ascii_chars().take(100)));
    }

    #[test]
    fn test_std_rng_reseed() {
        let s = thread_rng().gen_iter::<usize>().take(256).collect::<Vec<usize>>();
        let mut r: StdRng = SeedableRng::from_seed(s.as_slice());
        let string1 = r.gen_ascii_chars().take(100).collect::<String>();

        r.reseed(s.as_slice());

        let string2 = r.gen_ascii_chars().take(100).collect::<String>();
        assert_eq!(string1, string2);
    }
}

#[cfg(test)]
static RAND_BENCH_N: u64 = 100;

#[cfg(test)]
mod bench {
    extern crate test;
    use self::test::{black_box, Bencher};
    use super::{XorShiftRng, StdRng, IsaacRng, Isaac64Rng, Rng, RAND_BENCH_N};
    use super::{OsRng, weak_rng};
    use std::mem::size_of;

    #[bench]
    fn rand_xorshift(b: &mut Bencher) {
        let mut rng: XorShiftRng = OsRng::new().unwrap().gen();
        b.iter(|| {
            for _ in 0..RAND_BENCH_N {
                black_box(rng.gen::<usize>());
            }
        });
        b.bytes = size_of::<usize>() as u64 * RAND_BENCH_N;
    }

    #[bench]
    fn rand_isaac(b: &mut Bencher) {
        let mut rng: IsaacRng = OsRng::new().unwrap().gen();
        b.iter(|| {
            for _ in 0..RAND_BENCH_N {
                black_box(rng.gen::<usize>());
            }
        });
        b.bytes = size_of::<usize>() as u64 * RAND_BENCH_N;
    }

    #[bench]
    fn rand_isaac64(b: &mut Bencher) {
        let mut rng: Isaac64Rng = OsRng::new().unwrap().gen();
        b.iter(|| {
            for _ in 0..RAND_BENCH_N {
                black_box(rng.gen::<usize>());
            }
        });
        b.bytes = size_of::<usize>() as u64 * RAND_BENCH_N;
    }

    #[bench]
    fn rand_std(b: &mut Bencher) {
        let mut rng = StdRng::new().unwrap();
        b.iter(|| {
            for _ in 0..RAND_BENCH_N {
                black_box(rng.gen::<usize>());
            }
        });
        b.bytes = size_of::<usize>() as u64 * RAND_BENCH_N;
    }

    #[bench]
    fn rand_shuffle_100(b: &mut Bencher) {
        let mut rng = weak_rng();
        let x : &mut [usize] = &mut [1; 100];
        b.iter(|| {
            rng.shuffle(x);
        })
    }
}
