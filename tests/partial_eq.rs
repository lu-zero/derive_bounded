use derive_bounded::{Clone, PartialEq};

trait Associate {
    type A: PartialEq + Clone;
    type B: PartialEq + Clone;
    type C: PartialEq + Clone;
    type D: Associate;
}

#[derive(Debug)]
struct Holder;

impl Associate for Holder {
    type A = usize;
    type B = String;
    type C = u32;
    type D = Another;
}

#[derive(Debug)]
struct Another;

impl Associate for Another {
    type A = u32;
    type B = String;
    type C = u32;
    type D = Holder;
}

#[derive(Clone, PartialEq)]
#[bounded_to(T::B, T::C, <T::D as Associate>::A)]
struct A<T: Associate> {
    a: T::A,
    b: B<T>,
}

#[derive(Clone, PartialEq)]
#[bounded_to(T::C)]
struct B<T: Associate> {
    b: T::B,
    c: C<T>,
}

#[derive(Clone, PartialEq, Debug)]
#[bounded_to(T::C)]
struct C<T: Associate> {
    c: T::C,
}

#[derive(Clone, PartialEq, Debug)]
#[bounded_to(T::C)]
struct C2<T, V, Blah: Default>
where
    T: Associate,
{
    c: T::C,
    v: V,
    b: Blah,
}

#[test]
fn partial_eq() {
    let c = C { c: 42 };
    let a = A::<Holder> {
        a: 42,
        b: B {
            b: "Ok".to_owned(),
            c: C { c: 42 },
        },
    };

    assert_eq!(c, a.b.c);
}
