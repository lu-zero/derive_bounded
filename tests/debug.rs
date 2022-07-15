use derive_bounded::{Clone, Debug, PartialEq};

trait Associate {
    type A: PartialEq + Clone + std::fmt::Debug;
    type B: PartialEq + Clone + std::fmt::Debug;
    type C: PartialEq + Clone + std::fmt::Debug;
}

#[derive(std::fmt::Debug)]
struct Holder;

impl Associate for Holder {
    type A = usize;
    type B = String;
    type C = u32;
}

#[derive(Clone, PartialEq, Debug)]
#[bounded_to(types(T::B, T::C))]
struct A<T: Associate> {
    a: T::A,
    b: B<T>,
}

#[derive(Clone, PartialEq, Debug)]
#[bounded_to(types(T::C))]
struct B<T: Associate> {
    b: T::B,
    c: C<T>,
}

#[derive(Clone, PartialEq, Debug)]
#[bounded_to(types(T::C))]
struct C<T: Associate> {
    c: T::C,
}

#[derive(Clone, PartialEq, Debug)]
#[bounded_to(types(T::C))]
struct C2<T, V, Blah: Default>
where
    T: Associate,
{
    c: T::C,
    v: V,
    b: Blah,
}

#[test]
fn debug() {
    let c = C { c: 42 };
    let a = A::<Holder> {
        a: 42,
        b: B {
            b: "Ok".to_owned(),
            c: C { c: 42 },
        },
    };

    assert_eq!(c, a.b.c);

    dbg!(&a);
    dbg!(&a.b);
    dbg!(&a.b.c);
}
