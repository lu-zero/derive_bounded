use derive_bounded::{Clone, Debug, Default, PartialEq};

trait Associate {
    type A: PartialEq + Clone + std::fmt::Debug + Default;
    type B: PartialEq + Clone + std::fmt::Debug + Default;
    type C: PartialEq + Clone + std::fmt::Debug + Default;
    type D: Associate;
}

#[derive(std::fmt::Debug)]
struct Holder;

impl Associate for Holder {
    type A = usize;
    type B = String;
    type C = u32;
    type D = Another;
}

#[derive(std::fmt::Debug)]
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
#[bounded_to(T::B, T::C, <T::D as Associate>::A)]
enum En<T: Associate> {
    A { a: T::A, b: B<T> },
    B(T::A, B<T>),
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

#[derive(Clone, PartialEq, Debug, Default)]
#[bounded_to(T::C, T::B)]
struct D<T: Associate>(T::C, T::B);

#[derive(Clone, Debug, PartialEq, Default)]
struct ABase<A>(A);

#[derive(Clone, Debug, PartialEq, Default)]
struct BBase<A> {
    a: A,
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
