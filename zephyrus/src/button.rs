use std::collections::HashMap;

use crate::{command::CommandFun, twilight_exports::Permissions};
/// A map of [buttons](self::Button).
pub type ButtonMap<D> = HashMap<&'static str, Button<D>>;

/// A button executed by the framework.
pub struct Button<D> {
    /// The name of the button.
    pub name: &'static str,
    /// The description of the buttons.
    pub description: &'static str,
    /// A pointer to this button function.
    pub fun: CommandFun<D>,
    /// The required permissions to use this button
    pub required_permissions: Option<Permissions>,
}

impl<D> Button<D> {
    /// Creates a new button.
    pub fn new(fun: CommandFun<D>) -> Self {
        Self {
            name: Default::default(),
            description: Default::default(),
            fun,
            required_permissions: Default::default(),
        }
    }

    /// Sets the button name.
    pub fn name(mut self, name: &'static str) -> Self {
        self.name = name;
        self
    }

    /// Sets the button description.
    pub fn description(mut self, description: &'static str) -> Self {
        self.description = description;
        self
    }

    pub fn required_permissions(mut self, permissions: Permissions) -> Self {
        self.required_permissions = Some(permissions);
        self
    }
}
