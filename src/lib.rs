// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A structure for holding a set of enum variants.
//!
//! This module defines a container which uses an efficient bit mask
//! representation to hold C-like enum variants.

use std::fmt;
use std::hash;
use std::marker::PhantomData;
use std::iter;
use std::ops;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
/// A specialized set implementation to use enum types.
pub struct EnumSet<E> {
    // We must maintain the invariant that no bits are set
    // for which no variant exists
    bits: u32,
    phantom: PhantomData<E>,
}

impl<E: CLike + fmt::Debug> fmt::Debug for EnumSet<E> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_set().entries(self).finish()
    }
}

impl<E: CLike> hash::Hash for EnumSet<E> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.bits.hash(state);
    }
}

/// An interface for casting C-like enum to `u32` and back.
///
/// The returned value must be no more than 31: `EnumSet` does not support more cases than this.
///
/// A typical implementation can be seen below:
///
/// ```
/// use enum_set::CLike;
/// use std::mem;
///
/// #[derive(Clone, Copy)]
/// #[repr(u32)]
/// enum Foo {
///     A, B, C
/// }
///
/// impl CLike for Foo {
///     fn to_u32(&self) -> u32 {
///         *self as u32
///     }
///
///     unsafe fn from_u32(v: u32) -> Foo {
///         mem::transmute(v)
///     }
/// }
/// ```
pub trait CLike {
    /// Converts a C-like enum to a `u32`. The value must be `<= 31`.
    fn to_u32(&self) -> u32;

    /// Converts a `u32` to a C-like enum. This method only needs to be safe
    /// for possible return values of `to_u32` of this trait.
    unsafe fn from_u32(u32) -> Self;
}

fn bit<E: CLike>(e: &E) -> u32 {
    let value = e.to_u32();
    assert!(value < 32, "EnumSet only supports up to {} variants.", 31);
    1 << value
}

impl<E: CLike> EnumSet<E> {
    /// Returns an empty `EnumSet`.
    pub fn new() -> Self {
        Self::new_with_bits(0)
    }

    fn new_with_bits(bits: u32) -> Self {
        EnumSet { bits: bits, phantom: PhantomData }
    }

    /// Returns the number of elements in the set.
    pub fn len(&self) -> usize {
        self.bits.count_ones() as usize
    }

    /// Checks if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.bits == 0
    }

    /// Removes all elements from the set.
    pub fn clear(&mut self) {
        self.bits = 0;
    }

    /// Returns `true` if the set has no elements in common with `other`.
    ///
    /// This is equivalent to checking for an empty intersection.
    pub fn is_disjoint(&self, other: &Self) -> bool {
        (self.bits & other.bits) == 0
    }

    /// Returns `true` if the set is a superset of `other`.
    pub fn is_superset(&self, other: &Self) -> bool {
        (self.bits & other.bits) == other.bits
    }

    /// Returns `true` if the set is a subset of `other`.
    pub fn is_subset(&self, other: &Self) -> bool {
        other.is_superset(self)
    }

    /// Returns the union of the set and `other`.
    pub fn union(&self, other: Self) -> Self {
        Self::new_with_bits(self.bits | other.bits)
    }

    /// Returns the intersection of the set and `other`.
    pub fn intersection(&self, other: Self) -> Self {
        Self::new_with_bits(self.bits & other.bits)
    }

    /// Returns the difference between the set and `other`.
    pub fn difference(&self, other: Self) -> Self {
        Self::new_with_bits(self.bits & !other.bits)
    }

    /// Returns the symmetric difference between the set and `other`.
    pub fn symmetric_difference(&self, other: Self) -> Self {
        Self::new_with_bits(self.bits ^ other.bits)
    }

    /// Adds the given value to the set.
    ///
    /// Returns `true` if the value was not already present in the set.
    pub fn insert(&mut self, value: E) -> bool {
        let result = !self.contains(&value);
        self.bits |= bit(&value);
        result
    }

    /// Removes a value from the set.
    ///
    /// Returns `true` if the value was present in the set.
    pub fn remove(&mut self, value: &E) -> bool {
        let result = self.contains(value);
        self.bits &= !bit(value);
        result
    }

    /// Returns `true` if the set contains the given value.
    pub fn contains(&self, value: &E) -> bool {
        (self.bits & bit(value)) != 0
    }

    /// Returns an iterator over the set's elements.
    pub fn iter(&self) -> Iter<E> {
        Iter { index: 0, bits: self.bits, phantom: PhantomData }
    }
}

impl<E: CLike> ops::Sub for EnumSet<E> {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        self.difference(other)
    }
}

impl<E: CLike> ops::BitOr for EnumSet<E> {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        self.union(other)
    }
}

impl<E: CLike> ops::BitAnd for EnumSet<E> {
    type Output = Self;

    fn bitand(self, other: Self) -> Self {
        self.intersection(other)
    }
}

impl<E: CLike> ops::BitXor for EnumSet<E> {
    type Output = Self;

    fn bitxor(self, other: Self) -> Self {
        self.symmetric_difference(other)
    }
}

#[derive(Clone)]
/// An iterator over an `EnumSet`.
pub struct Iter<E> {
    index: u32,
    bits: u32,
    phantom: PhantomData<*mut E>,
}

impl<E: CLike> Iterator for Iter<E> {
    type Item = E;

    fn next(&mut self) -> Option<E> {
        if self.bits == 0 {
            return None;
        }

        while (self.bits & 1) == 0 {
            self.index += 1;
            self.bits >>= 1;
        }

        // Safe because of the invariant that only valid bits are set (see
        // comment on the `bit` member of this struct).
        let elem = unsafe { CLike::from_u32(self.index) };
        self.index += 1;
        self.bits >>= 1;
        Some(elem)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let exact = self.bits.count_ones() as usize;
        (exact, Some(exact))
    }
}

impl<E: CLike> ExactSizeIterator for Iter<E> {}

impl<E: CLike> Default for EnumSet<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: CLike> iter::FromIterator<E> for EnumSet<E> {
    fn from_iter<I: IntoIterator<Item = E>>(iterator: I) -> Self {
        let mut ret = Self::new();
        ret.extend(iterator);
        ret
    }
}

impl<E: CLike> Extend<E> for EnumSet<E> {
    fn extend<I: IntoIterator<Item = E>>(&mut self, iter: I) {
        for element in iter {
            self.insert(element);
        }
    }
}

impl<'a, E: CLike> IntoIterator for &'a EnumSet<E> {
    type Item = E;
    type IntoIter = Iter<E>;
    fn into_iter(self) -> Iter<E> { self.iter() }
}

#[cfg(test)]
mod tests {
    use self::Foo::*;
    use std::mem;

    use super::{EnumSet, CLike};

    #[derive(Copy, Clone, PartialEq, Debug)]
    #[repr(u32)]
    enum Foo {
        A, B, C
    }

    impl CLike for Foo {
        fn to_u32(&self) -> u32 {
            *self as u32
        }

        unsafe fn from_u32(v: u32) -> Foo {
            mem::transmute(v)
        }
    }

    #[test]
    fn test_new() {
        let e: EnumSet<Foo> = EnumSet::new();
        assert!(e.is_empty());
    }

    #[test]
    fn test_debug() {
        let mut e = EnumSet::new();
        assert_eq!("{}", format!("{:?}", e));
        e.insert(A);
        assert_eq!("{A}", format!("{:?}", e));
        e.insert(C);
        assert_eq!("{A, C}", format!("{:?}", e));
    }

    #[test]
    fn test_len() {
        let mut e = EnumSet::new();
        assert_eq!(e.len(), 0);
        e.insert(A);
        e.insert(B);
        e.insert(C);
        assert_eq!(e.len(), 3);
        e.remove(&A);
        assert_eq!(e.len(), 2);
        e.clear();
        assert_eq!(e.len(), 0);
    }

    ///////////////////////////////////////////////////////////////////////////
    // intersect

    #[test]
    fn test_two_empties_do_not_intersect() {
        let e1: EnumSet<Foo> = EnumSet::new();
        let e2: EnumSet<Foo> = EnumSet::new();
        assert!(e1.is_disjoint(&e2));
    }

    #[test]
    fn test_empty_does_not_intersect_with_full() {
        let e1: EnumSet<Foo> = EnumSet::new();

        let mut e2: EnumSet<Foo> = EnumSet::new();
        e2.insert(A);
        e2.insert(B);
        e2.insert(C);

        assert!(e1.is_disjoint(&e2));
    }

    #[test]
    fn test_disjoint_intersects() {
        let mut e1: EnumSet<Foo> = EnumSet::new();
        e1.insert(A);

        let mut e2: EnumSet<Foo> = EnumSet::new();
        e2.insert(B);

        assert!(e1.is_disjoint(&e2));
    }

    #[test]
    fn test_overlapping_intersects() {
        let mut e1: EnumSet<Foo> = EnumSet::new();
        e1.insert(A);

        let mut e2: EnumSet<Foo> = EnumSet::new();
        e2.insert(A);
        e2.insert(B);

        assert!(!e1.is_disjoint(&e2));
    }

    ///////////////////////////////////////////////////////////////////////////
    // contains and contains_elem

    #[test]
    fn test_superset() {
        let mut e1: EnumSet<Foo> = EnumSet::new();
        e1.insert(A);

        let mut e2: EnumSet<Foo> = EnumSet::new();
        e2.insert(A);
        e2.insert(B);

        let mut e3: EnumSet<Foo> = EnumSet::new();
        e3.insert(C);

        assert!(e1.is_subset(&e2));
        assert!(e2.is_superset(&e1));
        assert!(!e3.is_superset(&e2));
        assert!(!e2.is_superset(&e3));
    }

    #[test]
    fn test_contains() {
        let mut e1: EnumSet<Foo> = EnumSet::new();
        e1.insert(A);
        assert!(e1.contains(&A));
        assert!(!e1.contains(&B));
        assert!(!e1.contains(&C));

        e1.insert(A);
        e1.insert(B);
        assert!(e1.contains(&A));
        assert!(e1.contains(&B));
        assert!(!e1.contains(&C));
    }

    ///////////////////////////////////////////////////////////////////////////
    // iter

    #[test]
    fn test_iterator() {
        let mut e1: EnumSet<Foo> = EnumSet::new();

        let elems: Vec<Foo> = e1.iter().collect();
        assert!(elems.is_empty());

        e1.insert(A);
        let elems: Vec<_> = e1.iter().collect();
        assert_eq!(vec![A], elems);

        e1.insert(C);
        let elems: Vec<_> = e1.iter().collect();
        assert_eq!(vec![A,C], elems);

        e1.insert(C);
        let elems: Vec<_> = e1.iter().collect();
        assert_eq!(vec![A,C], elems);

        e1.insert(B);
        let elems: Vec<_> = e1.iter().collect();
        assert_eq!(vec![A,B,C], elems);
    }

    #[test]
    fn test_clone_iterator() {
        let mut e: EnumSet<Foo> = EnumSet::new();
        e.insert(A);
        e.insert(B);
        e.insert(C);

        let mut iter1 = e.iter();
        let first_elem = iter1.next();
        assert_eq!(Some(A), first_elem);

        let iter2 = iter1.clone();
        let elems1: Vec<_> = iter1.collect();
        assert_eq!(vec![B, C], elems1);

        let elems2: Vec<_> = iter2.collect();
        assert_eq!(vec![B, C], elems2);
    }

    ///////////////////////////////////////////////////////////////////////////
    // operators

    #[test]
    fn test_operators() {
        let mut e1: EnumSet<Foo> = EnumSet::new();
        e1.insert(A);
        e1.insert(C);

        let mut e2: EnumSet<Foo> = EnumSet::new();
        e2.insert(B);
        e2.insert(C);

        let e_union = e1 | e2;
        let elems: Vec<_> = e_union.iter().collect();
        assert_eq!(vec![A,B,C], elems);

        let e_intersection = e1 & e2;
        let elems: Vec<_> = e_intersection.iter().collect();
        assert_eq!(vec![C], elems);

        // Another way to express intersection
        let e_intersection = e1 - (e1 - e2);
        let elems: Vec<_> = e_intersection.iter().collect();
        assert_eq!(vec![C], elems);

        let e_subtract = e1 - e2;
        let elems: Vec<_> = e_subtract.iter().collect();
        assert_eq!(vec![A], elems);

        // Bitwise XOR of two sets, aka symmetric difference
        let e_symmetric_diff = e1 ^ e2;
        let elems: Vec<_> = e_symmetric_diff.iter().collect();
        assert_eq!(vec![A,B], elems);

        // Another way to express symmetric difference
        let e_symmetric_diff = (e1 - e2) | (e2 - e1);
        let elems: Vec<_> = e_symmetric_diff.iter().collect();
        assert_eq!(vec![A,B], elems);

        // Yet another way to express symmetric difference
        let e_symmetric_diff = (e1 | e2) - (e1 & e2);
        let elems: Vec<_> = e_symmetric_diff.iter().collect();
        assert_eq!(vec![A,B], elems);
    }

    #[test]
    #[should_panic]
    fn test_overflow() {
        #[allow(dead_code)]
        #[repr(u32)]
        #[derive(Clone, Copy)]
        enum Bar {
            V00, V01, V02, V03, V04, V05, V06, V07, V08, V09,
            V10, V11, V12, V13, V14, V15, V16, V17, V18, V19,
            V20, V21, V22, V23, V24, V25, V26, V27, V28, V29,
            V30, V31, V32, V33, V34, V35, V36, V37, V38, V39,
        }

        impl CLike for Bar {
            fn to_u32(&self) -> u32 {
                *self as u32
            }

            unsafe fn from_u32(v: u32) -> Bar {
                mem::transmute(v)
            }
        }

        let mut set = EnumSet::new();
        set.insert(Bar::V32);
    }
}
