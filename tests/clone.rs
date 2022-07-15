use derive_bounded::Clone;

trait Associate {
    type A: Clone;
    type B: Clone;
    type C: Clone;
}

#[derive(Debug)]
struct Holder;

impl Associate for Holder {
    type A = usize;
    type B = String;
    type C = u32;
}

#[derive(Clone)]
#[bounded_to(types(T::B, T::C))]
struct A<T: Associate> {
    a: T::A,
    b: B<T>,
}

#[derive(Clone)]
#[bounded_to(types(T::C))]
struct B<T: Associate> {
    b: T::B,
    c: C<T>,
}

#[derive(Clone)]
#[bounded_to(types(T::C))]
struct C<T: Associate> {
    c: T::C,
}

#[derive(Clone, Debug)]
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
fn clone() {
    let c2 = C2::<Holder, usize, usize> {
        c: 22,
        v: 42,
        b: Default::default(),
    };

    let d = c2.clone();

    dbg!(&d);
}
