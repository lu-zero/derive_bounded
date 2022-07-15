use derive_bounded::Default;

trait Associate {
    type A: Default;
    type B: Default;
    type C: Default;
}

#[derive(Debug)]
struct Holder;

impl Associate for Holder {
    type A = usize;
    type B = String;
    type C = u32;
}

#[derive(Default)]
#[bounded_to(T::B, T::C)]
struct A<T: Associate> {
    a: T::A,
    b: B<T>,
}

#[derive(Default)]
#[bounded_to(T::C)]
struct B<T: Associate> {
    b: T::B,
    c: C<T>,
}

#[derive(Default)]
#[bounded_to(T::C)]
struct C<T: Associate> {
    c: T::C,
}

#[derive(Default, Debug)]
#[bounded_to(T::C)]
struct C2<T, V, Blah: Clone>
where
    T: Associate,
{
    c: T::C,
    v: V,
    b: Blah,
}

#[test]
fn default() {
    let c2 = C2::<Holder, usize, usize>::default();

    dbg!(&c2);
}
