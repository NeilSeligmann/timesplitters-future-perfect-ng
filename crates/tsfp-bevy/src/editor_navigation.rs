//! Owned Map storage return contract (0x32B2B0 frontier). The caller supplies
//! real retained pages and consumes an adopted document; no guest context stack.
use crate::storage_caller::{NativePageId, ProfileNavigation, RetainedPage};
use tsfp_engine_fileio::{
    map_document::MapDocument,
    storage_flow::{MapSource, Operation},
    storage_frontend::{Completion, Target},
    vfs::{NtStatus, status},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorOperation {
    Load,
    Save,
}
impl EditorOperation {
    pub fn accepts(self, operation: Operation) -> bool {
        matches!(
            (self, operation),
            (Self::Load, Operation::MapLoad) | (Self::Save, Operation::MapSave)
        )
    }
}

/// A successful return carries the actual adopted source and document. Cancel
/// carries no new document and cannot be mistaken for a completed load/save.
#[derive(Clone, Debug, PartialEq)]
pub enum EditorReturn {
    Completed {
        destination: RetainedPage,
        operation: EditorOperation,
        source: MapSource,
        document: MapDocument,
    },
    Cancelled {
        destination: RetainedPage,
    },
}

pub struct EditorNavigation {
    navigation: ProfileNavigation,
    operation: EditorOperation,
    pending: bool,
    returned: Option<EditorReturn>,
}
impl EditorNavigation {
    pub fn new(
        operation: EditorOperation,
        history: Vec<RetainedPage>,
        completed: NativePageId,
        cancelled: NativePageId,
    ) -> Result<Self, NtStatus> {
        Ok(Self {
            navigation: ProfileNavigation::new(history, completed, cancelled)?,
            operation,
            pending: true,
            returned: None,
        })
    }
    pub fn pending(&self) -> bool {
        self.pending
    }
    pub fn operation(&self) -> EditorOperation {
        self.operation
    }
    pub fn has_return(&self) -> bool {
        self.returned.is_some()
    }
    pub fn pending_return(&self) -> Option<&EditorReturn> {
        self.returned.as_ref()
    }
    pub fn active(&self) -> Option<&RetainedPage> {
        self.navigation.active()
    }
    pub fn complete(
        &mut self,
        receipt: &Completion,
        source: &MapSource,
        document: &MapDocument,
    ) -> Result<(), NtStatus> {
        if !self.pending
            || receipt.target != Target::Map
            || !self.operation.accepts(receipt.operation)
            || match source {
                MapSource::Local(key) => receipt.key.as_ref() != Some(key),
                MapSource::DiscSample(_) => {
                    receipt.key.is_some() || self.operation != EditorOperation::Load
                }
            }
        {
            return Err(status::INVALID_PARAMETER);
        }
        document.serialize()?;
        self.navigation.finish(true)?;
        self.returned = Some(EditorReturn::Completed {
            destination: self
                .navigation
                .active()
                .ok_or(status::INVALID_PARAMETER)?
                .clone(),
            operation: self.operation,
            source: source.clone(),
            document: document.clone(),
        });
        self.pending = false;
        Ok(())
    }
    pub fn cancel(&mut self) -> Result<(), NtStatus> {
        if !self.pending {
            return Err(status::INVALID_PARAMETER);
        }
        self.navigation.finish(false)?;
        self.returned = Some(EditorReturn::Cancelled {
            destination: self
                .navigation
                .active()
                .ok_or(status::INVALID_PARAMETER)?
                .clone(),
        });
        self.pending = false;
        Ok(())
    }
    pub fn take_return(&mut self) -> Option<EditorReturn> {
        self.returned.take()
    }
}
