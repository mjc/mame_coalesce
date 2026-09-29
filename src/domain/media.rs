use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Read},
};

use serde::{Serialize, Serializer};
use sha2::{Digest, Sha256};

use super::SnapshotKey;

macro_rules! string_identity {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        pub struct $name(String);

        impl $name {
            #[must_use]
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

string_identity!(SoftwareListName);
string_identity!(SoftwareItemName);
string_identity!(SoftwarePartName);
string_identity!(SoftwareAreaName);
string_identity!(SoftwareComponentName);
string_identity!(RecipeImplementationName);
string_identity!(RecipeImplementationVersion);
string_identity!(RecipeParameterName);
string_identity!(CompatibilityTarget);
string_identity!(CompatibilityRunId);
string_identity!(ReferenceArtifactId);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct SoftwareItemKey {
    snapshot: SnapshotKey,
    list: SoftwareListName,
    item: SoftwareItemName,
}

impl SoftwareItemKey {
    #[must_use]
    pub const fn new(
        snapshot: SnapshotKey,
        list: SoftwareListName,
        item: SoftwareItemName,
    ) -> Self {
        Self {
            snapshot,
            list,
            item,
        }
    }

    #[must_use]
    pub const fn snapshot(&self) -> &SnapshotKey {
        &self.snapshot
    }

    #[must_use]
    pub const fn list(&self) -> &SoftwareListName {
        &self.list
    }

    #[must_use]
    pub const fn item(&self) -> &SoftwareItemName {
        &self.item
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct SoftwarePartKey {
    item: SoftwareItemKey,
    part: SoftwarePartName,
}

impl SoftwarePartKey {
    #[must_use]
    pub const fn new(item: SoftwareItemKey, part: SoftwarePartName) -> Self {
        Self { item, part }
    }

    #[must_use]
    pub const fn item(&self) -> &SoftwareItemKey {
        &self.item
    }

    #[must_use]
    pub const fn part(&self) -> &SoftwarePartName {
        &self.part
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct ComponentOrder(u32);

impl ComponentOrder {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct AreaOrder(u32);

impl AreaOrder {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct RecipeInputOrder(u32);

impl RecipeInputOrder {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum MediaAreaKind {
    Data,
    Disk,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct MediaComponentKey {
    part: SoftwarePartKey,
    area: SoftwareAreaName,
    area_order: AreaOrder,
    area_kind: MediaAreaKind,
    name: Option<SoftwareComponentName>,
    order: ComponentOrder,
}

impl MediaComponentKey {
    #[must_use]
    pub const fn new(
        part: SoftwarePartKey,
        area: SoftwareAreaName,
        area_order: AreaOrder,
        area_kind: MediaAreaKind,
        name: Option<SoftwareComponentName>,
        order: ComponentOrder,
    ) -> Self {
        Self {
            part,
            area,
            area_order,
            area_kind,
            name,
            order,
        }
    }

    #[must_use]
    pub const fn part(&self) -> &SoftwarePartKey {
        &self.part
    }

    #[must_use]
    pub const fn order(&self) -> ComponentOrder {
        self.order
    }

    #[must_use]
    pub const fn area(&self) -> &SoftwareAreaName {
        &self.area
    }

    #[must_use]
    pub const fn area_order(&self) -> AreaOrder {
        self.area_order
    }

    #[must_use]
    pub const fn area_kind(&self) -> MediaAreaKind {
        self.area_kind
    }

    #[must_use]
    pub const fn name(&self) -> Option<&SoftwareComponentName> {
        self.name.as_ref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum MediaComponentRole {
    Rom,
    Disk,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum SourceLoadInstruction {
    Load16Byte,
    Load16Word,
    Load16WordSwap,
    Load32Byte,
    Load32Word,
    Load32WordSwap,
    Load32Dword,
    Load64Word,
    Load64WordSwap,
    Reload,
    Fill,
    Continue,
    ReloadPlain,
    Ignore,
}

impl SourceLoadInstruction {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Load16Byte => "load16_byte",
            Self::Load16Word => "load16_word",
            Self::Load16WordSwap => "load16_word_swap",
            Self::Load32Byte => "load32_byte",
            Self::Load32Word => "load32_word",
            Self::Load32WordSwap => "load32_word_swap",
            Self::Load32Dword => "load32_dword",
            Self::Load64Word => "load64_word",
            Self::Load64WordSwap => "load64_word_swap",
            Self::Reload => "reload",
            Self::Fill => "fill",
            Self::Continue => "continue",
            Self::ReloadPlain => "reload_plain",
            Self::Ignore => "ignore",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum MediaDigestValue {
    Crc32([u8; 4]),
    Md5([u8; 16]),
    Sha1([u8; 20]),
    Sha256([u8; 32]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum MediaDigestScope {
    WholeContainer,
    LogicalMedia,
    ChdHeaderSha1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum MediaContentDigest {
    WholeContainer(MediaDigestValue),
    LogicalMedia(MediaDigestValue),
    ChdHeaderSha1([u8; 20]),
}

impl MediaContentDigest {
    #[must_use]
    pub const fn whole_container(value: MediaDigestValue) -> Self {
        Self::WholeContainer(value)
    }

    #[must_use]
    pub const fn logical_media(value: MediaDigestValue) -> Self {
        Self::LogicalMedia(value)
    }

    #[must_use]
    pub const fn chd_header_sha1(value: [u8; 20]) -> Self {
        Self::ChdHeaderSha1(value)
    }

    #[must_use]
    pub const fn scope(self) -> MediaDigestScope {
        match self {
            Self::WholeContainer(_) => MediaDigestScope::WholeContainer,
            Self::LogicalMedia(_) => MediaDigestScope::LogicalMedia,
            Self::ChdHeaderSha1(_) => MediaDigestScope::ChdHeaderSha1,
        }
    }

    #[must_use]
    pub const fn value(self) -> MediaDigestValue {
        match self {
            Self::WholeContainer(value) | Self::LogicalMedia(value) => value,
            Self::ChdHeaderSha1(value) => MediaDigestValue::Sha1(value),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepresentationDigest([u8; 32]);

impl RepresentationDigest {
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    pub fn from_reader(mut reader: impl Read) -> io::Result<Self> {
        let mut digest = Sha256::new();
        let mut buffer = [0; 16 * 1024];
        loop {
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            digest.update(&buffer[..count]);
        }
        Ok(Self(digest.finalize().into()))
    }

    #[must_use]
    pub const fn from_sha256(value: [u8; 32]) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Serialize for RepresentationDigest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&hex::encode(self.0))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct MediaByteLength(u64);

impl MediaByteLength {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RecipeInput {
    key: MediaComponentKey,
    order: RecipeInputOrder,
    role: MediaComponentRole,
    content: Option<MediaContentDigest>,
    observed_bytes: Option<RepresentationDigest>,
    byte_length: Option<MediaByteLength>,
    source_offset: Option<MediaByteLength>,
    source_load: Option<SourceLoadClaim>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MediaComponentFacts {
    key: MediaComponentKey,
    role: MediaComponentRole,
    content: Option<MediaContentDigest>,
    byte_length: Option<MediaByteLength>,
    source_offset: Option<MediaByteLength>,
    source_load: Option<SourceLoadClaim>,
}

impl MediaComponentFacts {
    #[must_use]
    pub const fn new(key: MediaComponentKey, role: MediaComponentRole) -> Self {
        Self {
            key,
            role,
            content: None,
            byte_length: None,
            source_offset: None,
            source_load: None,
        }
    }

    #[must_use]
    pub const fn with_content(mut self, content: MediaContentDigest) -> Self {
        self.content = Some(content);
        self
    }

    #[must_use]
    pub const fn with_byte_length(mut self, byte_length: MediaByteLength) -> Self {
        self.byte_length = Some(byte_length);
        self
    }

    #[must_use]
    pub const fn with_source_offset(mut self, source_offset: MediaByteLength) -> Self {
        self.source_offset = Some(source_offset);
        self
    }

    #[must_use]
    pub fn with_source_load(mut self, source_load: SourceLoadClaim) -> Self {
        self.source_load = Some(source_load);
        self
    }

    #[must_use]
    pub const fn key(&self) -> &MediaComponentKey {
        &self.key
    }

    #[must_use]
    pub const fn role(&self) -> MediaComponentRole {
        self.role
    }

    #[must_use]
    pub const fn content(&self) -> Option<MediaContentDigest> {
        self.content
    }

    #[must_use]
    pub const fn byte_length(&self) -> Option<MediaByteLength> {
        self.byte_length
    }

    #[must_use]
    pub const fn source_offset(&self) -> Option<MediaByteLength> {
        self.source_offset
    }

    #[must_use]
    pub const fn source_load(&self) -> Option<&SourceLoadClaim> {
        self.source_load.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SourceLoadClaim {
    instruction: SourceLoadInstruction,
    fill_value: Option<String>,
}

impl SourceLoadClaim {
    #[must_use]
    pub fn from_source(instruction: SourceLoadInstruction, value: Option<String>) -> Self {
        let fill_value = (instruction == SourceLoadInstruction::Fill)
            .then_some(value)
            .flatten();
        Self {
            instruction,
            fill_value,
        }
    }

    #[must_use]
    pub const fn instruction(&self) -> SourceLoadInstruction {
        self.instruction
    }

    #[must_use]
    pub fn fill_value(&self) -> Option<&str> {
        self.fill_value.as_deref()
    }
}

impl RecipeInput {
    #[must_use]
    pub const fn new(
        key: MediaComponentKey,
        order: RecipeInputOrder,
        role: MediaComponentRole,
    ) -> Self {
        Self {
            key,
            order,
            role,
            content: None,
            observed_bytes: None,
            byte_length: None,
            source_offset: None,
            source_load: None,
        }
    }

    #[must_use]
    pub const fn with_content(mut self, content: MediaContentDigest) -> Self {
        self.content = Some(content);
        self
    }

    #[must_use]
    pub const fn with_observed_bytes(mut self, digest: RepresentationDigest) -> Self {
        self.observed_bytes = Some(digest);
        self
    }

    #[must_use]
    pub const fn with_byte_length(mut self, byte_length: MediaByteLength) -> Self {
        self.byte_length = Some(byte_length);
        self
    }

    #[must_use]
    pub const fn with_source_offset(mut self, source_offset: MediaByteLength) -> Self {
        self.source_offset = Some(source_offset);
        self
    }

    #[must_use]
    pub fn with_source_load(mut self, source_load: SourceLoadClaim) -> Self {
        self.source_load = Some(source_load);
        self
    }

    #[must_use]
    pub const fn key(&self) -> &MediaComponentKey {
        &self.key
    }

    #[must_use]
    pub fn facts(&self) -> MediaComponentFacts {
        MediaComponentFacts {
            key: self.key.clone(),
            role: self.role,
            content: self.content,
            byte_length: self.byte_length,
            source_offset: self.source_offset,
            source_load: self.source_load.clone(),
        }
    }

    #[must_use]
    pub const fn order(&self) -> RecipeInputOrder {
        self.order
    }

    #[must_use]
    pub const fn role(&self) -> MediaComponentRole {
        self.role
    }

    #[must_use]
    pub const fn content(&self) -> Option<MediaContentDigest> {
        self.content
    }

    #[must_use]
    pub const fn observed_bytes(&self) -> Option<RepresentationDigest> {
        self.observed_bytes
    }

    #[must_use]
    pub const fn source_offset(&self) -> Option<MediaByteLength> {
        self.source_offset
    }

    #[must_use]
    pub const fn byte_length(&self) -> Option<MediaByteLength> {
        self.byte_length
    }

    #[must_use]
    pub const fn source_load(&self) -> Option<&SourceLoadClaim> {
        self.source_load.as_ref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum RecipeOperation {
    Preserve,
    Assemble,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct RecipeDependency {
    source: SoftwareItemKey,
    target: SoftwareItemKey,
    kind: SourceDependencyKind,
}

impl RecipeDependency {
    #[must_use]
    pub const fn clone_of(source: SoftwareItemKey, target: SoftwareItemKey) -> Self {
        Self {
            source,
            target,
            kind: SourceDependencyKind::CloneOf,
        }
    }

    #[must_use]
    pub const fn source(&self) -> &SoftwareItemKey {
        &self.source
    }

    #[must_use]
    pub const fn target(&self) -> &SoftwareItemKey {
        &self.target
    }

    #[must_use]
    pub const fn kind(&self) -> SourceDependencyKind {
        self.kind
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum SourceDependencyKind {
    CloneOf,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum RecipeParameterValue {
    Boolean(bool),
    Integer(i64),
    Text(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct RecipeParameters(BTreeMap<RecipeParameterName, RecipeParameterValue>);

impl RecipeParameters {
    pub fn insert(
        &mut self,
        name: RecipeParameterName,
        value: RecipeParameterValue,
    ) -> Option<RecipeParameterValue> {
        self.0.insert(name, value)
    }

    #[must_use]
    pub fn get(&self, name: &RecipeParameterName) -> Option<&RecipeParameterValue> {
        self.0.get(name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RecipeImplementation {
    name: RecipeImplementationName,
    version: RecipeImplementationVersion,
}

impl RecipeImplementation {
    #[must_use]
    pub const fn new(name: RecipeImplementationName, version: RecipeImplementationVersion) -> Self {
        Self { name, version }
    }

    #[must_use]
    pub const fn name(&self) -> &RecipeImplementationName {
        &self.name
    }

    #[must_use]
    pub const fn version(&self) -> &RecipeImplementationVersion {
        &self.version
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct RepresentationKey([u8; 32]);

impl RepresentationKey {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VerifiedRepresentationIdentity {
    key: RepresentationKey,
    digest: RepresentationDigest,
}

impl VerifiedRepresentationIdentity {
    #[must_use]
    pub const fn key(self) -> RepresentationKey {
        self.key
    }

    #[must_use]
    pub const fn digest(self) -> RepresentationDigest {
        self.digest
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ExactOutputReference {
    recipe: RepresentationKey,
    artifact: ReferenceArtifactId,
    digest: RepresentationDigest,
}

impl ExactOutputReference {
    /// Binds a verified recipe to bytes read from an independently identified reference artifact.
    pub fn establish(
        recipe: &MediaRecipe<ValidatedMediaRecipe>,
        artifact: ReferenceArtifactId,
        reference_bytes: impl Read,
    ) -> Result<Self, MediaRecipeError> {
        validate_observed_inputs(recipe)?;
        Ok(Self {
            recipe: recipe.representation,
            artifact,
            digest: RepresentationDigest::from_reader(reference_bytes)?,
        })
    }

    #[must_use]
    pub const fn artifact(&self) -> &ReferenceArtifactId {
        &self.artifact
    }

    #[must_use]
    pub const fn digest(&self) -> RepresentationDigest {
        self.digest
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CompatibilityEvidence {
    target: CompatibilityTarget,
    emulator: RecipeImplementation,
    test_suite: RecipeImplementation,
    run_id: CompatibilityRunId,
}

impl CompatibilityEvidence {
    /// Records a successful external emulator test; this model does not run the emulator.
    #[must_use]
    pub const fn passed(
        target: CompatibilityTarget,
        emulator: RecipeImplementation,
        test_suite: RecipeImplementation,
        run_id: CompatibilityRunId,
    ) -> Self {
        Self {
            target,
            emulator,
            test_suite,
            run_id,
        }
    }

    #[must_use]
    pub const fn target(&self) -> &CompatibilityTarget {
        &self.target
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RecipeVerification(VerificationState);

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
enum VerificationState {
    Unverified,
    ExactReconstruction {
        recipe: RepresentationKey,
        reference: ReferenceArtifactId,
        expected: RepresentationDigest,
        output: RepresentationDigest,
    },
    EmulatorCompatible {
        recipe: RepresentationKey,
        evidence: CompatibilityEvidence,
        output: RepresentationDigest,
    },
}

impl RecipeVerification {
    const fn unverified() -> Self {
        Self(VerificationState::Unverified)
    }

    pub fn exact_reconstruction(
        recipe: &MediaRecipe<ValidatedMediaRecipe>,
        reference: &ExactOutputReference,
        output: impl Read,
    ) -> Result<Self, MediaRecipeError> {
        validate_observed_inputs(recipe)?;
        if reference.recipe != recipe.representation {
            return Err(MediaRecipeError::StaleReference);
        }
        let output = RepresentationDigest::from_reader(output)?;
        if reference.digest != output {
            return Err(MediaRecipeError::ExactOutputMismatch);
        }
        Ok(Self(VerificationState::ExactReconstruction {
            recipe: recipe.representation,
            reference: reference.artifact.clone(),
            expected: reference.digest,
            output,
        }))
    }

    pub fn record_emulator_compatibility(
        recipe: &MediaRecipe<ValidatedMediaRecipe>,
        evidence: CompatibilityEvidence,
        output: impl Read,
    ) -> Result<Self, MediaRecipeError> {
        validate_observed_inputs(recipe)?;
        let output = RepresentationDigest::from_reader(output)?;
        Ok(Self(VerificationState::EmulatorCompatible {
            recipe: recipe.representation,
            evidence,
            output,
        }))
    }

    const fn representation_key(&self) -> Option<RepresentationKey> {
        match &self.0 {
            VerificationState::Unverified => None,
            VerificationState::ExactReconstruction { recipe, .. }
            | VerificationState::EmulatorCompatible { recipe, .. } => Some(*recipe),
        }
    }

    fn verified_output_digest(&self, key: RepresentationKey) -> Option<RepresentationDigest> {
        match &self.0 {
            VerificationState::ExactReconstruction { recipe, output, .. }
            | VerificationState::EmulatorCompatible { recipe, output, .. }
                if *recipe == key =>
            {
                Some(*output)
            }
            VerificationState::Unverified
            | VerificationState::ExactReconstruction { .. }
            | VerificationState::EmulatorCompatible { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct UnvalidatedMediaRecipe;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ValidatedMediaRecipe;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MediaRecipe<State = UnvalidatedMediaRecipe> {
    part: SoftwarePartKey,
    operation: RecipeOperation,
    inputs: Vec<RecipeInput>,
    dependencies: Vec<RecipeDependency>,
    implementation: RecipeImplementation,
    parameters: RecipeParameters,
    representation: RepresentationKey,
    verification: RecipeVerification,
    #[serde(skip)]
    state: std::marker::PhantomData<State>,
}

#[derive(Serialize)]
struct RecipeDefinition<'a> {
    format_version: u16,
    part: &'a SoftwarePartKey,
    operation: RecipeOperation,
    inputs: &'a [RecipeInput],
    dependencies: &'a [RecipeDependency],
    implementation: &'a RecipeImplementation,
    parameters: &'a RecipeParameters,
}

const RECIPE_FINGERPRINT_FORMAT_VERSION: u16 = 1;

impl MediaRecipe<UnvalidatedMediaRecipe> {
    pub fn new(
        part: SoftwarePartKey,
        operation: RecipeOperation,
        inputs: Vec<RecipeInput>,
        dependencies: Vec<RecipeDependency>,
        implementation: RecipeImplementation,
        parameters: RecipeParameters,
    ) -> Result<Self, MediaRecipeError> {
        validate_dependencies(&part, &dependencies)?;
        validate_inputs(&part, operation, &inputs, &dependencies)?;
        let definition = RecipeDefinition {
            format_version: RECIPE_FINGERPRINT_FORMAT_VERSION,
            part: &part,
            operation,
            inputs: &inputs,
            dependencies: &dependencies,
            implementation: &implementation,
            parameters: &parameters,
        };
        let representation =
            RepresentationKey(Sha256::digest(serde_json::to_vec(&definition)?).into());

        Ok(Self {
            part,
            operation,
            inputs,
            dependencies,
            implementation,
            parameters,
            representation,
            verification: RecipeVerification::unverified(),
            state: std::marker::PhantomData,
        })
    }
}

impl MediaRecipe<UnvalidatedMediaRecipe> {
    pub fn validate_references(
        self,
        available_items: &BTreeSet<SoftwareItemKey>,
        available_parts: &BTreeSet<SoftwarePartKey>,
        available_components: &BTreeMap<MediaComponentKey, MediaComponentFacts>,
        asserted_dependencies: &BTreeSet<RecipeDependency>,
    ) -> Result<MediaRecipe<ValidatedMediaRecipe>, MediaRecipeError> {
        validate_recipe_references(
            &self,
            available_items,
            available_parts,
            available_components,
            asserted_dependencies,
        )?;
        let Self {
            part,
            operation,
            inputs,
            dependencies,
            implementation,
            parameters,
            representation,
            verification,
            state: _,
        } = self;
        Ok(MediaRecipe {
            part,
            operation,
            inputs,
            dependencies,
            implementation,
            parameters,
            representation,
            verification,
            state: std::marker::PhantomData,
        })
    }
}

impl<State> MediaRecipe<State> {
    #[must_use]
    pub const fn part(&self) -> &SoftwarePartKey {
        &self.part
    }

    #[must_use]
    pub const fn operation(&self) -> RecipeOperation {
        self.operation
    }

    #[must_use]
    pub fn inputs(&self) -> &[RecipeInput] {
        &self.inputs
    }

    #[must_use]
    pub fn dependencies(&self) -> &[RecipeDependency] {
        &self.dependencies
    }

    #[must_use]
    pub const fn representation(&self) -> RepresentationKey {
        self.representation
    }

    #[must_use]
    pub const fn implementation(&self) -> &RecipeImplementation {
        &self.implementation
    }

    #[must_use]
    pub const fn parameters(&self) -> &RecipeParameters {
        &self.parameters
    }

    #[must_use]
    pub const fn verification(&self) -> &RecipeVerification {
        &self.verification
    }
}

impl MediaRecipe<ValidatedMediaRecipe> {
    #[must_use]
    pub fn verified_identity(&self) -> Option<VerifiedRepresentationIdentity> {
        self.verification
            .verified_output_digest(self.representation)
            .map(|digest| VerifiedRepresentationIdentity {
                key: self.representation,
                digest,
            })
    }

    #[must_use]
    pub fn is_exactly_verified(&self) -> bool {
        matches!(
            self.verification.0,
            VerificationState::ExactReconstruction { recipe, .. }
                if recipe == self.representation
        )
    }

    #[must_use]
    pub fn is_compatible_for(&self, target: &CompatibilityTarget) -> bool {
        matches!(
            &self.verification.0,
            VerificationState::EmulatorCompatible {
                recipe,
                evidence,
                ..
            } if *recipe == self.representation && evidence.target() == target
        )
    }

    pub fn with_verification(
        mut self,
        verification: RecipeVerification,
    ) -> Result<Self, MediaRecipeError> {
        if verification.representation_key() != Some(self.representation) {
            return Err(MediaRecipeError::StaleVerification);
        }
        self.verification = verification;
        Ok(self)
    }
}

fn validate_recipe_references(
    recipe: &MediaRecipe<UnvalidatedMediaRecipe>,
    available_items: &BTreeSet<SoftwareItemKey>,
    available_parts: &BTreeSet<SoftwarePartKey>,
    available_components: &BTreeMap<MediaComponentKey, MediaComponentFacts>,
    asserted_dependencies: &BTreeSet<RecipeDependency>,
) -> Result<(), MediaRecipeError> {
    if !available_parts.contains(&recipe.part) {
        return Err(MediaRecipeError::MissingRecipePart(Box::new(
            recipe.part.clone(),
        )));
    }
    for dependency in &recipe.dependencies {
        if !available_items.contains(&dependency.target) {
            return Err(MediaRecipeError::MissingDependency(Box::new(
                dependency.target.clone(),
            )));
        }
        if !asserted_dependencies.contains(dependency) {
            return Err(MediaRecipeError::UnassertedDependency(Box::new(
                dependency.clone(),
            )));
        }
    }
    for input in &recipe.inputs {
        let Some(component) = available_components.get(&input.key) else {
            return Err(MediaRecipeError::MissingComponent(Box::new(
                input.key.clone(),
            )));
        };
        if component != &input.facts() {
            return Err(MediaRecipeError::ComponentEvidenceMismatch(Box::new(
                input.key.clone(),
            )));
        }
    }
    Ok(())
}

fn validate_observed_inputs(
    recipe: &MediaRecipe<ValidatedMediaRecipe>,
) -> Result<(), MediaRecipeError> {
    if recipe
        .inputs
        .iter()
        .any(|input| input.observed_bytes.is_none())
    {
        return Err(MediaRecipeError::UnobservedInput);
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum MediaRecipeError {
    #[error("a media recipe needs at least one input")]
    NoInputs,
    #[error("preserving an asset requires exactly one input")]
    PreserveInputCount,
    #[error("recipe input is not owned by this item or a declared dependency")]
    InputPartMismatch,
    #[error("recipe input role contradicts its normalized media-area kind")]
    ComponentRoleMismatch,
    #[error("recipe input order must be strictly increasing")]
    InputOrder,
    #[error("recipe contains the same media component more than once")]
    DuplicateInput,
    #[error("software item cannot depend on itself")]
    SelfDependency,
    #[error("dependency assertion does not originate from the recipe item")]
    DependencySourceMismatch,
    #[error("dependency target must share the recipe snapshot and software list")]
    CrossSnapshotDependency,
    #[error("media recipe contains the same dependency more than once")]
    DuplicateDependency,
    #[error("media recipe dependency is absent: {0:?}")]
    MissingDependency(Box<SoftwareItemKey>),
    #[error("media recipe output part is absent from the selected snapshot: {0:?}")]
    MissingRecipePart(Box<SoftwarePartKey>),
    #[error("source dependency has no matching catalog assertion: {0:?}")]
    UnassertedDependency(Box<RecipeDependency>),
    #[error("media recipe component is absent from the selected snapshot: {0:?}")]
    MissingComponent(Box<MediaComponentKey>),
    #[error("media recipe component facts disagree with the selected snapshot: {0:?}")]
    ComponentEvidenceMismatch(Box<MediaComponentKey>),
    #[error("verification requires observed identities for every recipe input")]
    UnobservedInput,
    #[error("exact reconstruction output does not match the expected representation")]
    ExactOutputMismatch,
    #[error("exact-output reference belongs to a different recipe representation")]
    StaleReference,
    #[error("verification belongs to a different recipe representation")]
    StaleVerification,
    #[error("could not read reconstructed output: {0}")]
    OutputRead(#[from] io::Error),
    #[error("could not fingerprint media recipe: {0}")]
    Fingerprint(#[from] serde_json::Error),
}

fn validate_inputs(
    part: &SoftwarePartKey,
    operation: RecipeOperation,
    inputs: &[RecipeInput],
    dependencies: &[RecipeDependency],
) -> Result<(), MediaRecipeError> {
    if inputs.is_empty() {
        return Err(MediaRecipeError::NoInputs);
    }
    if operation == RecipeOperation::Preserve && inputs.len() != 1 {
        return Err(MediaRecipeError::PreserveInputCount);
    }

    let mut previous_order = None;
    let mut keys = BTreeSet::new();
    for input in inputs {
        let owner = &input.key.part.item;
        if owner != &part.item
            && !dependencies
                .iter()
                .any(|dependency| &dependency.target == owner)
        {
            return Err(MediaRecipeError::InputPartMismatch);
        }
        let role_matches_area = matches!(
            (input.role, input.key.area_kind),
            (MediaComponentRole::Rom, MediaAreaKind::Data)
                | (MediaComponentRole::Disk, MediaAreaKind::Disk)
        );
        if !role_matches_area {
            return Err(MediaRecipeError::ComponentRoleMismatch);
        }
        if previous_order.is_some_and(|order| order >= input.order) {
            return Err(MediaRecipeError::InputOrder);
        }
        if !keys.insert(&input.key) {
            return Err(MediaRecipeError::DuplicateInput);
        }
        previous_order = Some(input.order);
    }
    Ok(())
}

fn validate_dependencies(
    part: &SoftwarePartKey,
    dependencies: &[RecipeDependency],
) -> Result<(), MediaRecipeError> {
    let mut unique = BTreeSet::new();
    for dependency in dependencies {
        if dependency.source != part.item {
            return Err(MediaRecipeError::DependencySourceMismatch);
        }
        if dependency.target == part.item {
            return Err(MediaRecipeError::SelfDependency);
        }
        if dependency.target.snapshot != part.item.snapshot
            || dependency.target.list != part.item.list
        {
            return Err(MediaRecipeError::CrossSnapshotDependency);
        }
        if !unique.insert(dependency) {
            return Err(MediaRecipeError::DuplicateDependency);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part_key() -> SoftwarePartKey {
        SoftwarePartKey::new(
            SoftwareItemKey::new(
                SnapshotKey::from_persisted("snapshot-a".to_owned()),
                SoftwareListName::new("console"),
                SoftwareItemName::new("game"),
            ),
            SoftwarePartName::new("cart"),
        )
    }

    fn input(
        part: &SoftwarePartKey,
        order: u32,
        role: MediaComponentRole,
        load: Option<SourceLoadInstruction>,
        content: Option<MediaContentDigest>,
    ) -> RecipeInput {
        input_with_observed(part, order, role, None, load, content)
    }

    fn input_with_observed(
        part: &SoftwarePartKey,
        order: u32,
        role: MediaComponentRole,
        observed_bytes: Option<RepresentationDigest>,
        load: Option<SourceLoadInstruction>,
        content: Option<MediaContentDigest>,
    ) -> RecipeInput {
        let area_kind = match role {
            MediaComponentRole::Rom => MediaAreaKind::Data,
            MediaComponentRole::Disk => MediaAreaKind::Disk,
        };
        let mut recipe_input = RecipeInput::new(
            MediaComponentKey::new(
                part.clone(),
                SoftwareAreaName::new("program"),
                AreaOrder::new(0),
                area_kind,
                Some(SoftwareComponentName::new(format!("part-{order}.bin"))),
                ComponentOrder::new(order),
            ),
            RecipeInputOrder::new(order),
            role,
        )
        .with_byte_length(MediaByteLength::new(4))
        .with_source_offset(MediaByteLength::new(u64::from(order) * 4));
        if let Some(content) = content {
            recipe_input = recipe_input.with_content(content);
        }
        if let Some(observed_bytes) = observed_bytes {
            recipe_input = recipe_input.with_observed_bytes(observed_bytes);
        }
        if let Some(instruction) = load {
            let fill_value =
                (instruction == SourceLoadInstruction::Fill).then(|| "0xff".to_owned());
            recipe_input = recipe_input
                .with_source_load(SourceLoadClaim::from_source(instruction, fill_value));
        }
        recipe_input
    }

    fn implementation() -> RecipeImplementation {
        RecipeImplementation::new(
            RecipeImplementationName::new("test-assembler"),
            RecipeImplementationVersion::new("1"),
        )
    }

    fn recipe(
        part: SoftwarePartKey,
        inputs: Vec<RecipeInput>,
        dependencies: Vec<RecipeDependency>,
    ) -> Result<MediaRecipe, MediaRecipeError> {
        MediaRecipe::new(
            part,
            RecipeOperation::Assemble,
            inputs,
            dependencies,
            implementation(),
            RecipeParameters::default(),
        )
    }

    fn validate(
        recipe: MediaRecipe<UnvalidatedMediaRecipe>,
    ) -> Result<MediaRecipe<ValidatedMediaRecipe>, MediaRecipeError> {
        let mut available_items = BTreeSet::from([recipe.part().item().clone()]);
        available_items.extend(
            recipe
                .dependencies()
                .iter()
                .map(|dependency| dependency.target().clone()),
        );
        let available_components = recipe
            .inputs()
            .iter()
            .map(|input| (input.key().clone(), input.facts()))
            .collect();
        let asserted_dependencies = recipe.dependencies().iter().cloned().collect();
        let available_parts = BTreeSet::from([recipe.part().clone()]);
        recipe.validate_references(
            &available_items,
            &available_parts,
            &available_components,
            &asserted_dependencies,
        )
    }

    #[test]
    fn recipe_keeps_order_roles_dependencies_and_source_load_claims() -> Result<(), MediaRecipeError>
    {
        let part = part_key();
        let dependency = SoftwareItemKey::new(
            part.item.snapshot.clone(),
            part.item.list.clone(),
            SoftwareItemName::new("shared-bios"),
        );
        let inputs = vec![
            input(
                &part,
                0,
                MediaComponentRole::Rom,
                Some(SourceLoadInstruction::Load16Byte),
                None,
            ),
            input(
                &part,
                1,
                MediaComponentRole::Disk,
                Some(SourceLoadInstruction::Continue),
                Some(MediaContentDigest::logical_media(MediaDigestValue::Sha1(
                    [7; 20],
                ))),
            ),
        ];
        let recipe = recipe(
            part.clone(),
            inputs,
            vec![RecipeDependency::clone_of(part.item, dependency.clone())],
        )?;

        assert_eq!(recipe.inputs()[0].order().get(), 0);
        assert_eq!(recipe.inputs()[1].order().get(), 1);
        assert_eq!(recipe.inputs()[0].role(), MediaComponentRole::Rom);
        assert_eq!(recipe.inputs()[1].role(), MediaComponentRole::Disk);
        assert_eq!(
            recipe.inputs()[0]
                .source_load()
                .map(SourceLoadClaim::instruction),
            Some(SourceLoadInstruction::Load16Byte)
        );
        assert_eq!(
            recipe.inputs()[1]
                .source_load()
                .map(SourceLoadClaim::instruction),
            Some(SourceLoadInstruction::Continue)
        );
        assert_eq!(recipe.dependencies()[0].target(), &dependency);
        Ok(())
    }

    #[test]
    fn recipe_fingerprint_is_ordered_and_changes_with_source_identity()
    -> Result<(), MediaRecipeError> {
        let part = part_key();
        let inputs = vec![
            input(&part, 0, MediaComponentRole::Rom, None, None),
            input(&part, 1, MediaComponentRole::Rom, None, None),
        ];
        let original = recipe(part.clone(), inputs.clone(), Vec::new())?;
        let repeated = recipe(part.clone(), inputs, Vec::new())?;
        let changed = recipe(
            part.clone(),
            vec![
                input(&part, 0, MediaComponentRole::Rom, None, None),
                input_with_observed(
                    &part,
                    1,
                    MediaComponentRole::Rom,
                    Some(RepresentationDigest::from_bytes(b"changed input")),
                    None,
                    Some(MediaContentDigest::whole_container(MediaDigestValue::Sha1(
                        [2; 20],
                    ))),
                ),
            ],
            Vec::new(),
        )?;

        assert_eq!(original.representation(), repeated.representation());
        assert_eq!(
            hex::encode(original.representation().as_bytes()),
            "d2dd4fa2b58738f96b8875db8b49db14089142c7f60299abb02669c46b65b6a8"
        );
        assert_ne!(original.representation(), changed.representation());
        Ok(())
    }

    #[test]
    fn exact_verification_is_bound_to_unchanged_inputs_and_is_not_compatibility()
    -> Result<(), MediaRecipeError> {
        let part = part_key();
        let original = validate(recipe(
            part.clone(),
            vec![
                input_with_observed(
                    &part,
                    0,
                    MediaComponentRole::Rom,
                    Some(RepresentationDigest::from_bytes(b"input one")),
                    None,
                    None,
                ),
                input_with_observed(
                    &part,
                    1,
                    MediaComponentRole::Rom,
                    Some(RepresentationDigest::from_bytes(b"input two")),
                    None,
                    None,
                ),
            ],
            Vec::new(),
        )?)?;
        let reference = ExactOutputReference::establish(
            &original,
            ReferenceArtifactId::new("reference-set-v1"),
            std::io::Cursor::new(b"exact output"),
        )?;
        let verification = RecipeVerification::exact_reconstruction(
            &original,
            &reference,
            std::io::Cursor::new(b"exact output"),
        )?;
        let verified = original.with_verification(verification.clone())?;
        assert!(verified.is_exactly_verified());
        let changed = validate(recipe(
            part.clone(),
            vec![
                input_with_observed(
                    &part,
                    0,
                    MediaComponentRole::Rom,
                    Some(RepresentationDigest::from_bytes(b"input one")),
                    None,
                    None,
                ),
                input_with_observed(
                    &part,
                    1,
                    MediaComponentRole::Rom,
                    Some(RepresentationDigest::from_bytes(b"changed input")),
                    None,
                    Some(MediaContentDigest::logical_media(MediaDigestValue::Sha1(
                        [9; 20],
                    ))),
                ),
            ],
            Vec::new(),
        )?)?;
        assert!(!changed.is_exactly_verified());
        assert!(matches!(
            RecipeVerification::exact_reconstruction(
                &changed,
                &reference,
                std::io::Cursor::new(b"exact output"),
            ),
            Err(MediaRecipeError::StaleReference)
        ));
        assert!(matches!(
            changed.with_verification(verification),
            Err(MediaRecipeError::StaleVerification)
        ));

        let target = CompatibilityTarget::new("emulator-v1");
        let evidence = CompatibilityEvidence::passed(
            target.clone(),
            RecipeImplementation::new(
                RecipeImplementationName::new("mame"),
                RecipeImplementationVersion::new("0.280"),
            ),
            RecipeImplementation::new(
                RecipeImplementationName::new("software-list-smoke"),
                RecipeImplementationVersion::new("1"),
            ),
            CompatibilityRunId::new("run-17"),
        );
        let compatible_evidence = RecipeVerification::record_emulator_compatibility(
            &verified,
            evidence,
            std::io::Cursor::new(b"compatible output"),
        )?;
        let compatible = verified.with_verification(compatible_evidence)?;
        assert!(!compatible.is_exactly_verified());
        assert!(compatible.is_compatible_for(&target));
        Ok(())
    }

    #[test]
    fn verified_representation_identity_binds_recipe_key_to_output_digest()
    -> Result<(), MediaRecipeError> {
        let part = part_key();
        let original = validate(recipe(
            part.clone(),
            vec![input_with_observed(
                &part,
                0,
                MediaComponentRole::Rom,
                Some(RepresentationDigest::from_bytes(b"input")),
                None,
                None,
            )],
            Vec::new(),
        )?)?;
        let reference = ExactOutputReference::establish(
            &original,
            ReferenceArtifactId::new("reference-set-v1"),
            std::io::Cursor::new(b"exact output"),
        )?;
        let verification = RecipeVerification::exact_reconstruction(
            &original,
            &reference,
            std::io::Cursor::new(b"exact output"),
        )?;
        let verified = original.with_verification(verification)?;
        let identity = verified.verified_identity();
        assert_eq!(
            identity.map(VerifiedRepresentationIdentity::key),
            Some(verified.representation())
        );
        assert_eq!(
            identity.map(VerifiedRepresentationIdentity::digest),
            Some(RepresentationDigest::from_bytes(b"exact output"))
        );
        Ok(())
    }

    #[test]
    fn verification_requires_observed_inputs_and_matching_exact_output()
    -> Result<(), MediaRecipeError> {
        let part = part_key();
        let unobserved = validate(recipe(
            part.clone(),
            vec![input(&part, 0, MediaComponentRole::Rom, None, None)],
            Vec::new(),
        )?)?;
        assert!(matches!(
            ExactOutputReference::establish(
                &unobserved,
                ReferenceArtifactId::new("reference"),
                std::io::Cursor::new(b"output"),
            ),
            Err(MediaRecipeError::UnobservedInput)
        ));

        let observed = validate(recipe(
            part.clone(),
            vec![input_with_observed(
                &part,
                0,
                MediaComponentRole::Rom,
                Some(RepresentationDigest::from_bytes(b"input")),
                None,
                None,
            )],
            Vec::new(),
        )?)?;
        let reference = ExactOutputReference::establish(
            &observed,
            ReferenceArtifactId::new("reference"),
            std::io::Cursor::new(b"expected"),
        )?;
        assert!(matches!(
            RecipeVerification::exact_reconstruction(
                &observed,
                &reference,
                std::io::Cursor::new(b"actual"),
            ),
            Err(MediaRecipeError::ExactOutputMismatch)
        ));
        Ok(())
    }

    #[test]
    fn one_flat_rom_is_a_direct_representation_without_transformation()
    -> Result<(), MediaRecipeError> {
        let part = part_key();
        let recipe = validate(MediaRecipe::new(
            part.clone(),
            RecipeOperation::Preserve,
            vec![input(&part, 0, MediaComponentRole::Rom, None, None)],
            Vec::new(),
            implementation(),
            RecipeParameters::default(),
        )?)?;

        assert_eq!(recipe.operation(), RecipeOperation::Preserve);
        assert_eq!(recipe.inputs().len(), 1);
        assert!(!recipe.is_exactly_verified());
        Ok(())
    }

    #[test]
    fn missing_source_dependencies_are_explicit_errors() -> Result<(), MediaRecipeError> {
        let part = part_key();
        let dependency = SoftwareItemKey::new(
            part.item.snapshot.clone(),
            part.item.list.clone(),
            SoftwareItemName::new("shared-bios"),
        );
        let recipe = recipe(
            part.clone(),
            vec![
                input(&part_key(), 0, MediaComponentRole::Rom, None, None),
                input(&part_key(), 1, MediaComponentRole::Rom, None, None),
            ],
            vec![RecipeDependency::clone_of(part.item, dependency.clone())],
        )?;

        let available_components = recipe
            .inputs()
            .iter()
            .map(|input| (input.key().clone(), input.facts()))
            .collect();
        let available_parts = BTreeSet::from([recipe.part().clone()]);
        assert!(matches!(
            recipe.validate_references(
                &BTreeSet::new(),
                &available_parts,
                &available_components,
                &BTreeSet::new()
            ),
            Err(MediaRecipeError::MissingDependency(missing)) if *missing == dependency
        ));
        Ok(())
    }

    #[test]
    fn dependency_edges_are_scoped_to_the_recipe_item_and_snapshot() {
        let part = part_key();
        let target = SoftwareItemKey::new(
            part.item.snapshot.clone(),
            part.item.list.clone(),
            SoftwareItemName::new("parent"),
        );
        let other_source = SoftwareItemKey::new(
            part.item.snapshot.clone(),
            part.item.list.clone(),
            SoftwareItemName::new("unrelated"),
        );
        let cross_snapshot = SoftwareItemKey::new(
            SnapshotKey::from_persisted("snapshot-b".to_owned()),
            part.item.list.clone(),
            SoftwareItemName::new("parent"),
        );
        let source_item = part.item.clone();
        let input = vec![input(&part, 0, MediaComponentRole::Rom, None, None)];

        assert!(matches!(
            MediaRecipe::new(
                part.clone(),
                RecipeOperation::Preserve,
                input.clone(),
                vec![RecipeDependency::clone_of(other_source, target)],
                implementation(),
                RecipeParameters::default(),
            ),
            Err(MediaRecipeError::DependencySourceMismatch)
        ));
        assert!(matches!(
            MediaRecipe::new(
                part,
                RecipeOperation::Preserve,
                input,
                vec![RecipeDependency::clone_of(source_item, cross_snapshot)],
                implementation(),
                RecipeParameters::default(),
            ),
            Err(MediaRecipeError::CrossSnapshotDependency)
        ));
    }

    #[test]
    fn reference_validation_rejects_missing_components() -> Result<(), MediaRecipeError> {
        let part = part_key();
        let input_part = part.clone();
        let item = part.item.clone();
        let recipe = recipe(
            part,
            vec![input(&input_part, 0, MediaComponentRole::Rom, None, None)],
            Vec::new(),
        )?;
        assert!(matches!(
            recipe.validate_references(
                &BTreeSet::from([item]),
                &BTreeSet::from([input_part]),
                &BTreeMap::new(),
                &BTreeSet::new(),
            ),
            Err(MediaRecipeError::MissingComponent(_))
        ));
        Ok(())
    }

    #[test]
    fn reference_validation_rejects_component_fact_scope_mismatches() -> Result<(), MediaRecipeError>
    {
        let part = part_key();
        let input = input(
            &part,
            0,
            MediaComponentRole::Disk,
            Some(SourceLoadInstruction::Continue),
            Some(MediaContentDigest::whole_container(MediaDigestValue::Sha1(
                [4; 20],
            ))),
        );
        let recipe = recipe(part.clone(), vec![input.clone()], Vec::new())?;
        let mut snapshot_component = input.facts();
        snapshot_component.content = Some(MediaContentDigest::chd_header_sha1([4; 20]));

        assert!(matches!(
            recipe.validate_references(
                &BTreeSet::from([part.item.clone()]),
                &BTreeSet::from([part]),
                &BTreeMap::from([(input.key().clone(), snapshot_component)]),
                &BTreeSet::new(),
            ),
            Err(MediaRecipeError::ComponentEvidenceMismatch(_))
        ));
        Ok(())
    }

    #[test]
    fn digest_scopes_are_part_of_catalog_identity_and_output_has_its_own_type() {
        let whole = MediaContentDigest::whole_container(MediaDigestValue::Sha1([1; 20]));
        let logical = MediaContentDigest::logical_media(MediaDigestValue::Sha1([1; 20]));
        let chd_header = MediaContentDigest::chd_header_sha1([1; 20]);

        assert_ne!(whole, logical);
        assert_ne!(logical, chd_header);
        assert_eq!(whole.scope(), MediaDigestScope::WholeContainer);
        assert_eq!(logical.scope(), MediaDigestScope::LogicalMedia);
        assert_eq!(chd_header.scope(), MediaDigestScope::ChdHeaderSha1);
        assert_ne!(
            RepresentationDigest::from_bytes(b"bytes"),
            RepresentationDigest::from_bytes(b"different bytes")
        );
    }

    #[test]
    fn fill_load_claim_retains_its_operand() {
        let fill =
            SourceLoadClaim::from_source(SourceLoadInstruction::Fill, Some("0xff".to_owned()));
        let other = SourceLoadClaim::from_source(
            SourceLoadInstruction::Load16Byte,
            Some("ignored-source-value".to_owned()),
        );

        assert_eq!(fill.instruction(), SourceLoadInstruction::Fill);
        assert_eq!(fill.fill_value(), Some("0xff"));
        assert_eq!(other.fill_value(), None);
    }

    #[test]
    fn recipe_can_consume_an_explicit_dependency_component() -> Result<(), MediaRecipeError> {
        let part = part_key();
        let dependency = SoftwareItemKey::new(
            part.item.snapshot.clone(),
            part.item.list.clone(),
            SoftwareItemName::new("shared-bios"),
        );
        let dependency_part =
            SoftwarePartKey::new(dependency.clone(), SoftwarePartName::new("bios"));
        let dependency_edge = RecipeDependency::clone_of(part.item.clone(), dependency.clone());
        let recipe = recipe(
            part,
            vec![input(
                &dependency_part,
                0,
                MediaComponentRole::Rom,
                None,
                None,
            )],
            vec![dependency_edge.clone()],
        )?;

        assert_eq!(recipe.inputs()[0].key().part(), &dependency_part);
        let component_key = recipe.inputs()[0].key().clone();
        let component_facts = recipe.inputs()[0].facts();
        let available_parts = BTreeSet::from([recipe.part().clone()]);
        assert!(matches!(
            recipe.clone().validate_references(
                &BTreeSet::from([dependency.clone()]),
                &available_parts,
                &BTreeMap::from([(component_key.clone(), component_facts.clone())]),
                &BTreeSet::new(),
            ),
            Err(MediaRecipeError::UnassertedDependency(_))
        ));
        recipe.validate_references(
            &BTreeSet::from([dependency]),
            &available_parts,
            &BTreeMap::from([(component_key, component_facts)]),
            &BTreeSet::from([dependency_edge]),
        )?;
        Ok(())
    }

    #[test]
    fn repeated_named_areas_have_distinct_component_keys() {
        let part = part_key();
        let first = MediaComponentKey::new(
            part.clone(),
            SoftwareAreaName::new("program"),
            AreaOrder::new(0),
            MediaAreaKind::Data,
            Some(SoftwareComponentName::new("rom.bin")),
            ComponentOrder::new(0),
        );
        let repeated = MediaComponentKey::new(
            part,
            SoftwareAreaName::new("program"),
            AreaOrder::new(1),
            MediaAreaKind::Data,
            Some(SoftwareComponentName::new("rom.bin")),
            ComponentOrder::new(0),
        );

        assert_ne!(first, repeated);
    }

    #[test]
    fn recipe_rejects_role_and_area_kind_disagreement() {
        let part = part_key();
        let input = RecipeInput::new(
            MediaComponentKey::new(
                part.clone(),
                SoftwareAreaName::new("program"),
                AreaOrder::new(0),
                MediaAreaKind::Data,
                Some(SoftwareComponentName::new("disk.chd")),
                ComponentOrder::new(0),
            ),
            RecipeInputOrder::new(0),
            MediaComponentRole::Disk,
        );

        assert!(matches!(
            MediaRecipe::new(
                part,
                RecipeOperation::Preserve,
                vec![input],
                Vec::new(),
                implementation(),
                RecipeParameters::default(),
            ),
            Err(MediaRecipeError::ComponentRoleMismatch)
        ));
    }
}
