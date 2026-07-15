use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::pin::Pin;

pub mod collection_catalog;
pub mod craft_data;
pub mod item_icon;
pub mod weapon_model;

pub type ResourceFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

pub trait ResourceKindLabel: Clone + fmt::Debug + Eq + Hash + Send + Sync + 'static {
    fn id(&self) -> &'static str;
}

#[derive(Clone, Debug, Eq)]
pub struct ResourceKindKey {
    id: &'static str,
    debug_name: &'static str,
}

impl ResourceKindKey {
    pub fn new<L: ResourceKindLabel>(label: L) -> Self {
        Self {
            id: label.id(),
            debug_name: std::any::type_name::<L>(),
        }
    }

    pub fn id(&self) -> &'static str {
        self.id
    }

    pub fn debug_name(&self) -> &'static str {
        self.debug_name
    }
}

impl<L: ResourceKindLabel> From<L> for ResourceKindKey {
    fn from(label: L) -> Self {
        Self::new(label)
    }
}

impl PartialEq for ResourceKindKey {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Hash for ResourceKindKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl fmt::Display for ResourceKindKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResourceSource {
    /// Resources provided by xiv-companion itself, including bundled files and built-in network APIs.
    Builtin,
    /// Resources cached in the browser's IndexedDB, seeded from builtin data and optionally updated
    /// from a local game directory.
    IndexedDb,
    /// Resources explicitly supplied from the user's local machine, such as a game directory.
    UserLocal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResourceOrigin {
    Builtin,
    UserLocal,
    Network,
}

impl ResourceOrigin {
    pub fn id(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::UserLocal => "local",
            Self::Network => "network",
        }
    }
}

impl fmt::Display for ResourceOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

impl ResourceSource {
    pub fn id(&self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::IndexedDb => "indexed-db",
            Self::UserLocal => "user-local",
        }
    }
}

impl fmt::Display for ResourceSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceErrorKind {
    DecodeFailed,
    NotFound,
    NoSourceAvailable,
    PermissionMissing,
    ProviderFailed,
    Unsupported,
    VersionMismatch,
}

impl fmt::Display for ResourceErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::DecodeFailed => "decode failed",
            Self::NotFound => "not found",
            Self::NoSourceAvailable => "no source available",
            Self::PermissionMissing => "permission missing",
            Self::ProviderFailed => "provider failed",
            Self::Unsupported => "unsupported",
            Self::VersionMismatch => "version mismatch",
        };
        f.write_str(name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceError {
    pub kind: ResourceErrorKind,
    pub resource: ResourceKindKey,
    pub source: Option<ResourceSource>,
    pub message: String,
}

impl ResourceError {
    pub fn new(
        kind: ResourceErrorKind,
        resource: ResourceKindKey,
        source: Option<ResourceSource>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            resource,
            source,
            message: message.into(),
        }
    }

    fn summary(&self) -> String {
        match self.source {
            Some(source) => format!("{source}: {}", self.message),
            None => self.message.clone(),
        }
    }
}

impl fmt::Display for ResourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.source {
            Some(source) => write!(
                f,
                "{} from {source} failed: {}",
                self.resource, self.message
            ),
            None => write!(f, "{} failed: {}", self.resource, self.message),
        }
    }
}

impl std::error::Error for ResourceError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourcePolicy {
    Fallback(Vec<ResourceSource>),
    Fixed(ResourceSource),
}

impl SourcePolicy {
    fn sources(&self) -> Vec<ResourceSource> {
        match self {
            Self::Fallback(sources) => sources.clone(),
            Self::Fixed(source) => vec![*source],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FallbackPolicy {
    retryable_errors: Vec<ResourceErrorKind>,
}

impl FallbackPolicy {
    pub fn retrying(retryable_errors: impl Into<Vec<ResourceErrorKind>>) -> Self {
        Self {
            retryable_errors: retryable_errors.into(),
        }
    }

    pub fn can_try_next_source(&self, error: &ResourceError) -> bool {
        self.retryable_errors.contains(&error.kind)
    }
}

impl Default for FallbackPolicy {
    fn default() -> Self {
        Self::retrying([
            ResourceErrorKind::NotFound,
            ResourceErrorKind::PermissionMissing,
            ResourceErrorKind::Unsupported,
        ])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CachePolicy {
    None,
    ReadOnly,
    ReadWrite,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceDescriptor {
    pub kind: ResourceKindKey,
    pub default_policy: SourcePolicy,
    pub fallback_policy: FallbackPolicy,
    pub cache_policy: CachePolicy,
    pub pipeline: &'static str,
}

#[derive(Clone, Debug, Default)]
pub struct ResourceRegistry {
    descriptors: HashMap<ResourceKindKey, ResourceDescriptor>,
}

impl ResourceRegistry {
    pub fn register(&mut self, descriptor: ResourceDescriptor) {
        self.descriptors.insert(descriptor.kind.clone(), descriptor);
    }

    pub fn descriptor(&self, kind: &ResourceKindKey) -> Option<&ResourceDescriptor> {
        self.descriptors.get(kind)
    }
}

#[derive(Clone, Debug, Default)]
pub struct ResourceSettings {
    policies: HashMap<ResourceKindKey, SourcePolicy>,
}

impl ResourceSettings {
    pub fn set_policy(&mut self, kind: ResourceKindKey, policy: SourcePolicy) {
        self.policies.insert(kind, policy);
    }

    fn policy_for<'a>(&'a self, descriptor: &'a ResourceDescriptor) -> &'a SourcePolicy {
        self.policies
            .get(&descriptor.kind)
            .unwrap_or(&descriptor.default_policy)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderRequest {
    pub kind: ResourceKindKey,
    pub key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceBlob {
    pub bytes: Vec<u8>,
    pub fingerprint: Option<String>,
    pub metadata: ResourceMetadata,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResourceMetadata {
    pub origin: Option<ResourceOrigin>,
    pub game_version: Option<String>,
    pub revision: Option<String>,
    pub saved_at: Option<String>,
    pub record_count: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedResource<T> {
    pub source: ResourceSource,
    pub metadata: ResourceMetadata,
    pub value: T,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceStatus {
    pub resource: ResourceKindKey,
    pub storage: ResourceSource,
    pub available: bool,
    pub metadata: ResourceMetadata,
}

pub trait ResourceProvider {
    fn source(&self) -> ResourceSource;
    fn supports(&self, request: &ProviderRequest) -> bool;
    fn read<'a>(
        &'a self,
        request: ProviderRequest,
    ) -> ResourceFuture<'a, Result<ResourceBlob, ResourceError>>;

    fn status<'a>(
        &'a self,
        request: ProviderRequest,
    ) -> ResourceFuture<'a, Result<ResourceStatus, ResourceError>> {
        let source = self.source();
        Box::pin(async move {
            Err(ResourceError::new(
                ResourceErrorKind::Unsupported,
                request.kind,
                Some(source),
                "provider does not expose resource status",
            ))
        })
    }

    fn refresh<'a>(
        &'a self,
        request: ProviderRequest,
        origin: ResourceOrigin,
    ) -> ResourceFuture<'a, Result<ResourceStatus, ResourceError>> {
        let source = self.source();
        Box::pin(async move {
            Err(ResourceError::new(
                ResourceErrorKind::Unsupported,
                request.kind,
                Some(source),
                format!("provider cannot refresh from {origin}"),
            ))
        })
    }

    fn reset<'a>(
        &'a self,
        request: ProviderRequest,
    ) -> ResourceFuture<'a, Result<ResourceStatus, ResourceError>> {
        let source = self.source();
        Box::pin(async move {
            Err(ResourceError::new(
                ResourceErrorKind::Unsupported,
                request.kind,
                Some(source),
                "provider cannot reset this resource",
            ))
        })
    }
}

pub struct ResourceHub {
    registry: ResourceRegistry,
    providers: HashMap<ResourceSource, Vec<Box<dyn ResourceProvider>>>,
    settings: ResourceSettings,
}

impl ResourceHub {
    pub fn new() -> Self {
        Self {
            registry: ResourceRegistry::default(),
            providers: HashMap::new(),
            settings: ResourceSettings::default(),
        }
    }

    pub fn register_resource<R: ResourceSpec>(&mut self) {
        self.registry.register(R::descriptor());
    }

    pub fn add_provider(&mut self, provider: impl ResourceProvider + 'static) {
        self.providers
            .entry(provider.source())
            .or_default()
            .push(Box::new(provider));
    }

    pub fn set_policy(&mut self, kind: ResourceKindKey, policy: SourcePolicy) {
        self.settings.set_policy(kind, policy);
    }

    pub async fn load<R: ResourceSpec>(&self, id: R::Id) -> Result<R::Output, ResourceError> {
        self.load_with_source::<R>(id)
            .await
            .map(|loaded| loaded.value)
    }

    pub async fn load_with_source<R: ResourceSpec>(
        &self,
        id: R::Id,
    ) -> Result<LoadedResource<R::Output>, ResourceError> {
        let resource_kind = R::kind();
        let descriptor = self.descriptor(&resource_kind)?;
        let sources = self.settings.policy_for(descriptor).sources();
        if sources.is_empty() {
            return Err(ResourceError::new(
                ResourceErrorKind::NoSourceAvailable,
                resource_kind,
                None,
                "no source policy is configured",
            ));
        }

        let mut attempts = Vec::new();
        for source in sources {
            match self.load_from_with_metadata::<R>(source, id.clone()).await {
                Ok((value, metadata)) => {
                    return Ok(LoadedResource {
                        source,
                        metadata,
                        value,
                    });
                }
                Err(error) => {
                    let can_try_next = descriptor.fallback_policy.can_try_next_source(&error);
                    attempts.push(error.summary());
                    if !can_try_next {
                        return Err(error);
                    }
                }
            }
        }

        Err(ResourceError::new(
            ResourceErrorKind::NoSourceAvailable,
            resource_kind,
            None,
            format!("all configured sources failed ({})", attempts.join("; ")),
        ))
    }

    pub async fn load_from<R: ResourceSpec>(
        &self,
        source: ResourceSource,
        id: R::Id,
    ) -> Result<R::Output, ResourceError> {
        self.load_from_with_metadata::<R>(source, id)
            .await
            .map(|(value, _)| value)
    }

    async fn load_from_with_metadata<R: ResourceSpec>(
        &self,
        source: ResourceSource,
        id: R::Id,
    ) -> Result<(R::Output, ResourceMetadata), ResourceError> {
        let resource_kind = R::kind();
        self.descriptor(&resource_kind)?;

        let request = R::request(&id);
        let providers = self.providers.get(&source).ok_or_else(|| {
            ResourceError::new(
                ResourceErrorKind::Unsupported,
                resource_kind.clone(),
                Some(source),
                format!("source {source} is not registered"),
            )
        })?;

        for provider in providers {
            if !provider.supports(&request) {
                continue;
            }
            let blob = provider.read(request.clone()).await?;
            let context = DecodeContext {
                resource: resource_kind,
                source,
                fingerprint: blob.fingerprint,
            };
            let value = R::decode(blob.bytes, context)?;
            return Ok((value, blob.metadata));
        }

        Err(ResourceError::new(
            ResourceErrorKind::Unsupported,
            resource_kind.clone(),
            Some(source),
            format!("source {source} cannot provide {resource_kind}"),
        ))
    }

    pub async fn status<R: ResourceSpec>(
        &self,
        id: R::Id,
    ) -> Result<ResourceStatus, ResourceError> {
        let request = R::request(&id);
        self.management_provider(&request)?.status(request).await
    }

    pub async fn refresh<R: ResourceSpec>(
        &self,
        id: R::Id,
        origin: ResourceOrigin,
    ) -> Result<ResourceStatus, ResourceError> {
        let request = R::request(&id);
        self.management_provider(&request)?
            .refresh(request, origin)
            .await
    }

    pub async fn reset<R: ResourceSpec>(&self, id: R::Id) -> Result<ResourceStatus, ResourceError> {
        let request = R::request(&id);
        self.management_provider(&request)?.reset(request).await
    }

    fn management_provider(
        &self,
        request: &ProviderRequest,
    ) -> Result<&dyn ResourceProvider, ResourceError> {
        self.providers
            .get(&ResourceSource::IndexedDb)
            .and_then(|providers| providers.iter().find(|provider| provider.supports(request)))
            .map(|provider| provider.as_ref())
            .ok_or_else(|| {
                ResourceError::new(
                    ResourceErrorKind::Unsupported,
                    request.kind.clone(),
                    Some(ResourceSource::IndexedDb),
                    "resource does not expose managed storage operations",
                )
            })
    }

    fn descriptor(&self, kind: &ResourceKindKey) -> Result<&ResourceDescriptor, ResourceError> {
        self.registry.descriptor(kind).ok_or_else(|| {
            ResourceError::new(
                ResourceErrorKind::Unsupported,
                kind.clone(),
                None,
                format!("resource {kind} is not registered"),
            )
        })
    }
}

impl Default for ResourceHub {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodeContext {
    pub resource: ResourceKindKey,
    pub source: ResourceSource,
    pub fingerprint: Option<String>,
}

pub trait ResourceSpec {
    type Id: Clone;
    type Output;

    fn kind() -> ResourceKindKey;
    fn descriptor() -> ResourceDescriptor;
    fn request(id: &Self::Id) -> ProviderRequest;
    fn decode(bytes: Vec<u8>, context: DecodeContext) -> Result<Self::Output, ResourceError>;
}

pub fn compare_resource_versions(left: &str, right: &str) -> std::cmp::Ordering {
    resource_version_components(left).cmp(&resource_version_components(right))
}

fn resource_version_components(value: &str) -> Vec<u64> {
    value
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u64>().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    struct TestKind;

    impl ResourceKindLabel for TestKind {
        fn id(&self) -> &'static str {
            "test.resource"
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum TestId {
        Ok,
        Invalid,
    }

    struct TestResource;

    impl ResourceSpec for TestResource {
        type Id = TestId;
        type Output = String;

        fn kind() -> ResourceKindKey {
            TestKind.into()
        }

        fn descriptor() -> ResourceDescriptor {
            ResourceDescriptor {
                kind: Self::kind(),
                default_policy: SourcePolicy::Fallback(vec![
                    ResourceSource::Builtin,
                    ResourceSource::UserLocal,
                ]),
                fallback_policy: FallbackPolicy::default(),
                cache_policy: CachePolicy::None,
                pipeline: "test-v1",
            }
        }

        fn request(id: &Self::Id) -> ProviderRequest {
            let key = match id {
                TestId::Ok => "ok",
                TestId::Invalid => "invalid",
            };
            ProviderRequest {
                kind: Self::kind(),
                key: key.to_string(),
            }
        }

        fn decode(bytes: Vec<u8>, context: DecodeContext) -> Result<Self::Output, ResourceError> {
            String::from_utf8(bytes).map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::DecodeFailed,
                    context.resource,
                    Some(context.source),
                    format!("invalid UTF-8: {error}"),
                )
            })
        }
    }

    struct TestProvider {
        source: ResourceSource,
        error: Option<ResourceErrorKind>,
        value: &'static [u8],
    }

    impl ResourceProvider for TestProvider {
        fn source(&self) -> ResourceSource {
            self.source
        }

        fn supports(&self, request: &ProviderRequest) -> bool {
            request.kind == TestResource::kind()
        }

        fn read<'a>(
            &'a self,
            request: ProviderRequest,
        ) -> ResourceFuture<'a, Result<ResourceBlob, ResourceError>> {
            Box::pin(async move {
                if let Some(kind) = self.error {
                    return Err(ResourceError::new(
                        kind,
                        request.kind,
                        Some(self.source),
                        format!("test {kind}"),
                    ));
                }

                let bytes = match request.key.as_str() {
                    "invalid" => vec![0xff],
                    _ => self.value.to_vec(),
                };
                Ok(ResourceBlob {
                    bytes,
                    fingerprint: Some("test".to_string()),
                    metadata: ResourceMetadata {
                        origin: Some(if self.source == ResourceSource::Builtin {
                            ResourceOrigin::Builtin
                        } else {
                            ResourceOrigin::UserLocal
                        }),
                        game_version: Some("game-2026.01.01.0000.0000".to_string()),
                        ..Default::default()
                    },
                })
            })
        }

        fn status<'a>(
            &'a self,
            request: ProviderRequest,
        ) -> ResourceFuture<'a, Result<ResourceStatus, ResourceError>> {
            Box::pin(async move {
                Ok(ResourceStatus {
                    resource: request.kind,
                    storage: self.source,
                    available: true,
                    metadata: ResourceMetadata {
                        origin: Some(ResourceOrigin::Builtin),
                        game_version: Some("game-2026.01.01.0000.0000".to_string()),
                        ..Default::default()
                    },
                })
            })
        }
    }

    fn test_hub(first_error: Option<ResourceErrorKind>) -> ResourceHub {
        let mut hub = ResourceHub::new();
        hub.register_resource::<TestResource>();
        hub.add_provider(TestProvider {
            source: ResourceSource::Builtin,
            error: first_error,
            value: b"builtin",
        });
        hub.add_provider(TestProvider {
            source: ResourceSource::UserLocal,
            error: None,
            value: b"user-local",
        });
        hub.add_provider(TestProvider {
            source: ResourceSource::IndexedDb,
            error: None,
            value: b"indexed-db",
        });
        hub
    }

    #[test]
    fn load_uses_first_available_source() {
        let value = futures_executor::block_on(test_hub(None).load::<TestResource>(TestId::Ok))
            .expect("resource should load");

        assert_eq!(value, "builtin");
    }

    #[test]
    fn load_falls_back_for_retryable_errors() {
        let value = futures_executor::block_on(
            test_hub(Some(ResourceErrorKind::NotFound)).load::<TestResource>(TestId::Ok),
        )
        .expect("resource should load from fallback");

        assert_eq!(value, "user-local");
    }

    #[test]
    fn load_from_uses_requested_source_without_fallback() {
        let value = futures_executor::block_on(
            test_hub(Some(ResourceErrorKind::NotFound))
                .load_from::<TestResource>(ResourceSource::UserLocal, TestId::Ok),
        )
        .expect("resource should load from requested source");

        assert_eq!(value, "user-local");
    }

    #[test]
    fn load_with_source_reports_selected_fallback_source() {
        let loaded = futures_executor::block_on(
            test_hub(Some(ResourceErrorKind::NotFound))
                .load_with_source::<TestResource>(TestId::Ok),
        )
        .expect("resource should load from fallback");

        assert_eq!(loaded.source, ResourceSource::UserLocal);
        assert_eq!(loaded.metadata.origin, Some(ResourceOrigin::UserLocal));
        assert_eq!(loaded.value, "user-local");
    }

    #[test]
    fn load_does_not_fallback_for_decode_errors() {
        let error =
            futures_executor::block_on(test_hub(None).load::<TestResource>(TestId::Invalid))
                .expect_err("decode errors should stop fallback");

        assert_eq!(error.kind, ResourceErrorKind::DecodeFailed);
        assert_eq!(error.source, Some(ResourceSource::Builtin));
    }

    #[test]
    fn resource_versions_compare_numeric_components() {
        assert!(
            compare_resource_versions("game-2026.10.01.0000.0000", "game-2026.9.30.0000.0000")
                .is_gt()
        );
        assert!(compare_resource_versions("Local SqPack", "unknown").is_eq());
    }

    #[test]
    fn status_uses_managed_indexed_db_provider() {
        let status = futures_executor::block_on(test_hub(None).status::<TestResource>(TestId::Ok))
            .expect("managed status should load");
        assert_eq!(status.storage, ResourceSource::IndexedDb);
        assert_eq!(status.metadata.origin, Some(ResourceOrigin::Builtin));
    }
}
