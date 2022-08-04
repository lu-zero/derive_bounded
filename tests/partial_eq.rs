use derive_bounded::{Clone, Debug, Default, Eq, PartialEq};

trait Associate {
    type A: PartialEq + Clone + std::fmt::Debug + Default + Eq;
    type B: PartialEq + Clone + std::fmt::Debug + Default + Eq;
    type C: PartialEq + Clone + std::fmt::Debug + Default + Eq;
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

#[derive(Clone, PartialEq, Eq)]
#[bounded_to(T::B, T::C, <T::D as Associate>::A)]
struct A<T: Associate> {
    a: T::A,
    b: B<T>,
}

#[derive(Clone, PartialEq, Eq)]
#[bounded_to(T::B, T::C, <T::D as Associate>::A)]
enum En<T: Associate> {
    A { a: T::A, b: B<T> },
    B(T::A, B<T>),
}
#[derive(Clone, PartialEq, Eq)]
#[bounded_to(T::C)]
struct B<T: Associate> {
    b: T::B,
    c: C<T>,
}

#[derive(Clone, PartialEq, Debug, Eq)]
#[bounded_to(T::C)]
struct C<T: Associate> {
    c: T::C,
}

#[derive(Clone, PartialEq, Debug, Eq)]
#[bounded_to(T::C)]
struct C2<T, V, Blah: Default>
where
    T: Associate,
{
    c: T::C,
    v: V,
    b: Blah,
}

#[derive(Clone, PartialEq, Debug, Default, Eq)]
#[bounded_to(T::C, T::B)]
struct D<T: Associate>(T::C, T::B);

#[derive(Clone, Debug, PartialEq, Default, Eq)]
struct ABase<A>(A);

#[derive(Clone, Debug, PartialEq, Default, Eq)]
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
