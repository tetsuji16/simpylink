//! Block Parameters dialog.
use super::{DialogButton, DialogOutcome};
use sim_core::edit::Command;
use sim_core::ids::BlockId;
use sim_core::library::{spec, validate_param, ParamKind};
use sim_core::model::Model;
use sim_core::path::SystemPath;

/// One editable field.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamField {
    pub name: String,
    pub label: String,
    pub value: String,
    pub original: String,
    pub kind: ParamKind,
    pub error: Option<String>,
}

/// Dialog state.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamDialogState {
    pub path: SystemPath,
    pub block: BlockId,
    pub title: String,
    pub block_type: String,
    pub description: String,
    pub fields: Vec<ParamField>,
}

impl ParamDialogState {
    /// Opens for a block (None if unknown).
    pub fn open(model: &Model, path: &SystemPath, block: BlockId) -> Option<Self> {
        let b = model.system(path)?.block(block)?;
        let s = spec(&b.block_type)?;
        let fields = s
            .params
            .iter()
            .map(|p| {
                let v = b
                    .params
                    .get(p.name)
                    .cloned()
                    .unwrap_or_else(|| p.default.to_string());
                ParamField {
                    name: p.name.into(),
                    label: p.label.into(),
                    value: v.clone(),
                    original: v.clone(),
                    kind: p.kind,
                    error: validate_param(p.kind, &v).err(),
                }
            })
            .collect();
        Some(ParamDialogState {
            path: path.clone(),
            block,
            title: format!("Block Parameters: {}", b.name),
            block_type: b.block_type.clone(),
            description: s.description.into(),
            fields,
        })
    }
    /// Sets a field value and revalidates it.
    pub fn set_value(&mut self, index: usize, value: String) {
        if let Some(f) = self.fields.get_mut(index) {
            f.error = validate_param(f.kind, &value).err();
            f.value = value;
        }
    }
    /// True if any field is invalid.
    pub fn has_errors(&self) -> bool {
        self.fields.iter().any(|f| f.error.is_some())
    }
    /// Changed fields.
    pub fn changed(&self) -> Vec<(String, Option<String>)> {
        self.fields
            .iter()
            .filter(|f| f.value != f.original)
            .map(|f| (f.name.clone(), Some(f.value.clone())))
            .collect()
    }
    /// Button press.
    pub fn press(&mut self, button: DialogButton) -> DialogOutcome {
        match button {
            DialogButton::Cancel => DialogOutcome::Close,
            _ if self.has_errors() => DialogOutcome::KeepOpen,
            DialogButton::Ok => {
                let ch = self.changed();
                if ch.is_empty() {
                    DialogOutcome::Close
                } else {
                    DialogOutcome::CloseAndExecute(Command::SetParams {
                        path: self.path.clone(),
                        block: self.block,
                        params: ch,
                    })
                }
            }
            DialogButton::Apply => {
                let ch = self.changed();
                if ch.is_empty() {
                    return DialogOutcome::KeepOpen;
                }
                for f in &mut self.fields {
                    f.original = f.value.clone();
                }
                DialogOutcome::Execute(Command::SetParams {
                    path: self.path.clone(),
                    block: self.block,
                    params: ch,
                })
            }
        }
    }
}
