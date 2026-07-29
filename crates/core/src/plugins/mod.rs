//! `PluginRegistry`: the collection of active plugins that `scanner` and
//! `scheduler` iterate over.
//!
//! The `Plugin` trait itself lives in the separate `sentinel-plugin-api` crate,
//! not here — see that crate's docs for why.

use std::collections::HashSet;
use std::fmt;

use sentinel_plugin_api::{Plugin, PluginMetadata};

/// Why a plugin couldn't be registered.
#[derive(Debug, PartialEq, Eq)]
pub enum RegistryError {
    /// Two plugins claimed the same id.
    ///
    /// Ids are the lookup key and are written into cleanup history, so a
    /// collision would make it ambiguous which plugin cleaned what — and would
    /// let a lookup route a `clean` call to the wrong plugin entirely. Refusing
    /// at registration turns that into a startup failure instead of a
    /// mis-deletion later.
    DuplicateId {
        /// The id that was claimed twice.
        id: String,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateId { id } => {
                write!(formatter, "two plugins registered with the id {id:?}")
            }
        }
    }
}

impl std::error::Error for RegistryError {}

/// Holds every active plugin, keyed by id.
///
/// Deliberately a `Vec` rather than a `HashMap`: registration order is the
/// order the UI lists plugins in, and there will never be enough plugins for
/// linear lookup to matter. A separate `HashSet` of ids keeps duplicate
/// detection O(1) without giving up that ordering.
#[derive(Default)]
pub struct PluginRegistry {
    plugins: Vec<Box<dyn Plugin>>,
    ids: HashSet<&'static str>,
}

impl PluginRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a plugin, refusing a duplicate id.
    pub fn register(&mut self, plugin: Box<dyn Plugin>) -> Result<(), RegistryError> {
        let id = plugin.metadata().id;

        if !self.ids.insert(id) {
            return Err(RegistryError::DuplicateId { id: id.to_owned() });
        }

        self.plugins.push(plugin);
        Ok(())
    }

    /// Looks up a plugin by id.
    pub fn get(&self, id: &str) -> Option<&dyn Plugin> {
        self.plugins
            .iter()
            .find(|plugin| plugin.metadata().id == id)
            .map(AsRef::as_ref)
    }

    /// Every plugin, in registration order.
    pub fn iter(&self) -> impl Iterator<Item = &dyn Plugin> {
        self.plugins.iter().map(AsRef::as_ref)
    }

    /// Metadata for every registered plugin — enough to render a list without
    /// running a scan.
    pub fn metadata(&self) -> Vec<PluginMetadata> {
        self.plugins
            .iter()
            .map(|plugin| plugin.metadata())
            .collect()
    }

    /// How many plugins are registered.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Whether no plugins are registered.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

impl fmt::Debug for PluginRegistry {
    /// Lists ids rather than attempting to format `dyn Plugin`, which has no
    /// `Debug` bound — requiring one would burden every plugin author for the
    /// sake of a log line.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PluginRegistry")
            .field(
                "plugins",
                &self
                    .plugins
                    .iter()
                    .map(|p| p.metadata().id)
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestPlugin;

    #[test]
    fn new_registry_is_empty() {
        let registry = PluginRegistry::new();

        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn registers_and_looks_up_by_id() {
        let mut registry = PluginRegistry::new();
        registry
            .register(Box::new(TestPlugin::new("alpha")))
            .expect("first registration of an id succeeds");

        assert_eq!(registry.len(), 1);
        let found = registry
            .get("alpha")
            .expect("registered plugin is findable");
        assert_eq!(found.metadata().id, "alpha");
    }

    #[test]
    fn unknown_id_is_none() {
        let registry = PluginRegistry::new();

        assert!(registry.get("nope").is_none());
    }

    #[test]
    fn duplicate_ids_are_refused() {
        let mut registry = PluginRegistry::new();
        registry
            .register(Box::new(TestPlugin::new("alpha")))
            .expect("first registration succeeds");

        let error = registry
            .register(Box::new(TestPlugin::new("alpha")))
            .expect_err("a second plugin with the same id must be refused");

        assert_eq!(
            error,
            RegistryError::DuplicateId {
                id: "alpha".to_owned()
            }
        );
        assert_eq!(registry.len(), 1, "the duplicate must not be stored");
    }

    #[test]
    fn iteration_preserves_registration_order() {
        let mut registry = PluginRegistry::new();
        for id in ["alpha", "beta", "gamma"] {
            registry
                .register(Box::new(TestPlugin::new(id)))
                .expect("distinct ids register cleanly");
        }

        let ids: Vec<_> = registry.iter().map(|p| p.metadata().id).collect();

        assert_eq!(ids, vec!["alpha", "beta", "gamma"]);
    }
}
