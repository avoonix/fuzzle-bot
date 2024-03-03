use leptos::{create_signal, ReadSignal, SignalUpdate, WriteSignal};

#[derive(Clone)]
pub struct StickerContext {
    pub dragend: ReadSignal<i32>,
    set_dragend: WriteSignal<i32>,
}

impl StickerContext {
    pub fn new() -> Self {
        let (dragend, set_dragend) = create_signal(0);
        StickerContext {
            dragend,
            set_dragend,
        }
    }

    pub fn emit_dragend(&self) {
        self.set_dragend.update(|val| *val += 1);
    }
}
