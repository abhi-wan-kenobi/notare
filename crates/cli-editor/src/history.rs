const MAX_HISTORY: usize = 100;

pub(crate) struct Snapshot {
    pub lines: Vec<String>,
    pub row: usize,
    pub col: usize,
}

pub(crate) struct History {
    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
}

impl History {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn push(&mut self, snapshot: Snapshot) {
        if self.undo_stack.len() >= MAX_HISTORY {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(snapshot);
        self.redo_stack.clear();
    }

    pub fn undo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let prev = self.undo_stack.pop()?;
        self.redo_stack.push(current);
        Some(prev)
    }

    pub fn redo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let next = self.redo_stack.pop()?;
        self.undo_stack.push(current);
        Some(next)
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}
