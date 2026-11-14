//! Block Parameters dialog.
use super::{DialogButton, DialogOutcome};
use sim_core::edit::Command;
use sim_core::ids::BlockId;
use sim_core::library::{spec, validate_param, ParamKind};
use sim_core::model::Model;
use sim_core::path::SystemPath;
use std::collections::BTreeMap;

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
        let mut dialog = ParamDialogState {
            path: path.clone(),
            block,
            title: format!("Block Parameters: {}", b.name),
            block_type: b.block_type.clone(),
            description: s.description.into(),
            fields,
        };
        dialog.revalidate();
        Some(dialog)
    }
    /// Sets a field value and revalidates it.
    pub fn set_value(&mut self, index: usize, value: String) {
        if let Some(f) = self.fields.get_mut(index) {
            f.value = value;
        }
        self.revalidate();
    }
    /// Recomputes the error of every field, including the port-count limit.
    ///
    /// `validate_param` only checks a value on its own; `Mux` with `Inputs = 257` is a
    /// positive integer but would make `port_counts` fail, and the ports would then
    /// disappear from the canvas. Checking the block's ports here keeps the model and
    /// the drawing consistent.
    fn revalidate(&mut self) {
        for f in &mut self.fields {
            f.error = validate_param(f.kind, &f.value).err();
        }
        if let Some((index, message)) = self.port_count_error() {
            if let Some(f) = self.fields.get_mut(index) {
                f.error = Some(message);
            }
        }
    }
    /// The fields as a parameter map, as `port_counts` would see them.
    fn params(&self) -> BTreeMap<String, String> {
        self.fields
            .iter()
            .map(|f| (f.name.clone(), f.value.clone()))
            .collect()
    }
    /// The fields with their original values.
    fn original_params(&self) -> BTreeMap<String, String> {
        self.fields
            .iter()
            .map(|f| (f.name.clone(), f.original.clone()))
            .collect()
    }
    /// Field index and message when the edited values would give invalid port counts.
    ///
    /// The message is attributed to the field that causes it. Values that were not
    /// edited are never blamed, so a model that is already inconsistent can still be
    /// closed; but an edited value that is invalid on its own is always reported, even
    /// when the value it replaced was invalid too (257 -> 258 on a broken model).
    fn port_count_error(&self) -> Option<(usize, String)> {
        let ports = spec(&self.block_type)?.ports;
        let mut params = self.params();
        let Err(message) = ports(&params) else {
            return None;
        };
        // Prefer the field whose revert alone makes the block valid.
        for (index, f) in self.fields.iter().enumerate() {
            if f.value == f.original {
                continue;
            }
            params.insert(f.name.clone(), f.original.clone());
            let reverted_ok = ports(&params).is_ok();
            params.insert(f.name.clone(), f.value.clone());
            if reverted_ok {
                return Some((index, message));
            }
        }
        // Otherwise blame the first edited field that is invalid on its own.
        let original = self.original_params();
        for (index, f) in self.fields.iter().enumerate() {
            if f.value == f.original {
                continue;
            }
            let mut alone = original.clone();
            alone.insert(f.name.clone(), f.value.clone());
            if let Err(message) = ports(&alone) {
                return Some((index, message));
            }
        }
        None
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
