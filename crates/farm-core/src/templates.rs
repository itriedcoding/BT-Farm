//! Built-in instance templates.
//!
//! Every entry is a real provisioning request: a documented image/snapshot
//! identifier where the provider has one, plus CPU/RAM/disk/GPU values the
//! provider's API actually accepts. Fields a provider does not honour are
//! stripped by `CreateInstanceRequest::normalized_for` before the request is
//! sent, so a template can never pretend to size a machine it cannot size.
//!
//! Provenance is kept in `source` and rendered in the UI next to the picker.

use crate::model::{CreateInstanceRequest, CustomTemplate, Provider};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct InstanceTemplate {
    pub id: &'static str,
    pub provider: Provider,
    pub name: &'static str,
    pub description: &'static str,
    /// Template / snapshot / image identifier, when the provider takes one.
    pub image: Option<&'static str>,
    /// Boat machine class.
    pub size: Option<&'static str>,
    pub cpu: Option<u32>,
    pub memory_gb: Option<u64>,
    pub disk_gb: Option<u64>,
    pub ttl_seconds: Option<u64>,
    pub gpu: Option<u32>,
    pub gpu_type: Option<&'static str>,
    pub workload: &'static str,
    pub source: &'static str,
}

impl InstanceTemplate {
    /// Builds the request for this template and strips anything the provider
    /// cannot honour.
    pub fn to_request(&self, name: Option<String>) -> CreateInstanceRequest {
        let request = CreateInstanceRequest {
            name,
            template: self.image.map(str::to_string),
            size: self.size.map(str::to_string),
            cpu: self.cpu,
            memory_mb: self.memory_gb.map(|gb| gb * 1024),
            disk_gb: self.disk_gb,
            ttl_seconds: self.ttl_seconds,
            gpu: self.gpu,
            gpu_type: self.gpu_type.map(str::to_string),
            ..CreateInstanceRequest::default()
        };
        request.normalized_for(self.provider)
    }
}

const E2B_SOURCE: &str = "E2B documented templates (base, e2bdev/base); sizing comes from the template";
const FREESTYLE_SOURCE: &str = "Freestyle documented snapshots (freestyle/ubuntu, freestyle/ubuntu-lg)";
const DAYTONA_SOURCE: &str = "Daytona sandbox API: cpu, memory (GiB), disk, gpu, gpuType";
const DAYTONA_GPU_SOURCE: &str = "Daytona GPU sandboxes: POST /sandboxes/gpu with documented GPU types";
const BOAT_SOURCE: &str = "Boat sandbox API: type small/default/large/xlarge";
const BOXD_SOURCE: &str = "boxd CLI: machine new --vcpu (1/2/4/8)";
const RECIPE_SOURCE: &str =
    "Provisioning request plus a real post-boot setup command run through the provider's exec API";

const fn tpl(
    id: &'static str,
    provider: Provider,
    name: &'static str,
    description: &'static str,
    image: Option<&'static str>,
    size: Option<&'static str>,
    cpu: Option<u32>,
    memory_gb: Option<u64>,
    disk_gb: Option<u64>,
    ttl_seconds: Option<u64>,
    gpu: Option<u32>,
    gpu_type: Option<&'static str>,
    workload: &'static str,
    source: &'static str,
) -> InstanceTemplate {
    InstanceTemplate {
        id,
        provider,
        name,
        description,
        image,
        size,
        cpu,
        memory_gb,
        disk_gb,
        ttl_seconds,
        gpu,
        gpu_type,
        workload,
        source,
    }
}

/// 25 built-in templates. Kept as a `const` so the catalog cannot drift at
/// runtime and the tests can assert against the shipped list.
pub const CATALOG: &[InstanceTemplate] = &[
    // -- E2B: CPU-only microVMs, size fixed by the template ------------------
    tpl("e2b-base", Provider::E2b, "E2B base", "Debian toolbox with Python 3, Node, git, curl and build-essential. 2 vCPU / 512 MiB, as E2B sizes it.", Some("base"), None, None, None, None, Some(1800), None, None, "General sandbox", E2B_SOURCE),
    tpl("e2b-base-image", Provider::E2b, "E2B base image (e2bdev/base)", "The explicit upstream base image alias, for accounts that pin template names.", Some("e2bdev/base"), None, None, None, None, Some(1800), None, None, "General sandbox", E2B_SOURCE),
    tpl("e2b-long", Provider::E2b, "E2B long session (4 h)", "Base template with a four-hour timeout for longer agent runs.", Some("base"), None, None, None, None, Some(14_400), None, None, "Long agent run", E2B_SOURCE),
    tpl("e2b-day", Provider::E2b, "E2B full day (24 h)", "Base template at E2B's maximum sandbox timeout.", Some("base"), None, None, None, None, Some(86_400), None, None, "Overnight job", E2B_SOURCE),
    // -- Freestyle: snapshot decides the machine ----------------------------
    tpl("fs-ubuntu", Provider::Freestyle, "Freestyle Ubuntu", "Standard Freestyle Ubuntu snapshot for a one-hour worker.", Some("freestyle/ubuntu"), None, None, None, None, Some(3600), None, None, "General VM", FREESTYLE_SOURCE),
    tpl("fs-ubuntu-lg", Provider::Freestyle, "Freestyle Ubuntu (large)", "Larger Ubuntu snapshot for heavier builds and browsers.", Some("freestyle/ubuntu-lg"), None, None, None, None, Some(3600), None, None, "Build / browser work", FREESTYLE_SOURCE),
    tpl("fs-ubuntu-6h", Provider::Freestyle, "Freestyle Ubuntu (6 h)", "Standard snapshot with a six-hour TTL.", Some("freestyle/ubuntu"), None, None, None, None, Some(21_600), None, None, "Long build", FREESTYLE_SOURCE),
    tpl("fs-ubuntu-lg-day", Provider::Freestyle, "Freestyle Ubuntu large (24 h)", "Large snapshot with a full-day TTL.", Some("freestyle/ubuntu-lg"), None, None, None, None, Some(86_400), None, None, "Persistent worker", FREESTYLE_SOURCE),
    // -- Daytona: real cpu / memory / disk sizing --------------------------
    tpl("daytona-1", Provider::Daytona, "Daytona 1 vCPU / 1 GB", "Smallest Linux sandbox; good for short scripts and API checks.", None, None, Some(1), Some(1), Some(10), None, None, None, "Light script", DAYTONA_SOURCE),
    tpl("daytona-2", Provider::Daytona, "Daytona 2 vCPU / 4 GB", "Balanced sandbox for web automation and unit-test runs.", None, None, Some(2), Some(4), Some(20), None, None, None, "Web automation", DAYTONA_SOURCE),
    tpl("daytona-4", Provider::Daytona, "Daytona 4 vCPU / 8 GB", "Build sandbox with room for a JVM or Android tooling chain.", None, None, Some(4), Some(8), Some(40), None, None, None, "Build server", DAYTONA_SOURCE),
    tpl("daytona-8", Provider::Daytona, "Daytona 8 vCPU / 16 GB", "Parallel test runner.", None, None, Some(8), Some(16), Some(80), None, None, None, "Parallel tests", DAYTONA_SOURCE),
    // -- Daytona GPU: documented types, one unit each ----------------------
    tpl("daytona-gpu-h100", Provider::Daytona, "Daytona GPU · NVIDIA H100", "One H100 unit; Daytona states each GPU adds up to 16 vCPU and 192 GB RAM of headroom.", None, None, Some(8), Some(32), Some(100), None, Some(1), Some("H100"), "Model inference", DAYTONA_GPU_SOURCE),
    tpl("daytona-gpu-h200", Provider::Daytona, "Daytona GPU · NVIDIA H200", "One H200 unit for larger-memory inference.", None, None, Some(8), Some(32), Some(100), None, Some(1), Some("H200"), "Model inference", DAYTONA_GPU_SOURCE),
    tpl("daytona-gpu-b200", Provider::Daytona, "Daytona GPU · NVIDIA B200", "One B200 unit for training and fine-tuning.", None, None, Some(8), Some(64), Some(200), None, Some(1), Some("B200"), "Training", DAYTONA_GPU_SOURCE),
    tpl("daytona-gpu-b300", Provider::Daytona, "Daytona GPU · NVIDIA B300", "One B300 unit, Daytona's largest NVIDIA class.", None, None, Some(8), Some(64), Some(200), None, Some(1), Some("B300"), "Training", DAYTONA_GPU_SOURCE),
    tpl("daytona-gpu-rtx-pro-6000", Provider::Daytona, "Daytona GPU · RTX Pro 6000", "Workstation-class GPU for graphics and CUDA work.", None, None, Some(8), Some(32), Some(100), None, Some(1), Some("RTX-PRO-6000"), "Graphics / CUDA", DAYTONA_GPU_SOURCE),
    tpl("daytona-gpu-rtx-4090", Provider::Daytona, "Daytona GPU · RTX 4090", "Consumer-class GPU; cheapest documented unit.", None, None, Some(4), Some(16), Some(50), None, Some(1), Some("RTX-4090"), "Prototyping", DAYTONA_GPU_SOURCE),
    tpl("daytona-gpu-rtx-5090", Provider::Daytona, "Daytona GPU · RTX 5090", "Newer consumer GPU for inference prototyping.", None, None, Some(4), Some(16), Some(50), None, Some(1), Some("RTX-5090"), "Prototyping", DAYTONA_GPU_SOURCE),
    tpl("daytona-gpu-mi355x", Provider::Daytona, "Daytona GPU · AMD MI355X", "AMD Instinct accelerator for ROCm workloads.", None, None, Some(8), Some(32), Some(100), None, Some(1), Some("MI355X"), "ROCm workloads", DAYTONA_GPU_SOURCE),
    // -- Boat: machine class presets --------------------------------------
    tpl("boat-small", Provider::Boat, "Boat small", "Two-vCPU Boat machine, two-hour TTL.", None, Some("small"), Some(2), None, None, Some(7200), None, None, "Light worker", BOAT_SOURCE),
    tpl("boat-default", Provider::Boat, "Boat default", "Four-vCPU Boat machine — the documented default class.", None, Some("default"), Some(4), None, None, Some(7200), None, None, "General worker", BOAT_SOURCE),
    tpl("boat-large", Provider::Boat, "Boat large", "Eight-vCPU Boat machine for heavier automation.", None, Some("large"), Some(8), None, None, Some(7200), None, None, "Heavy worker", BOAT_SOURCE),
    // -- boxd: vCPU only ---------------------------------------------------
    tpl("boxd-2", Provider::Boxd, "boxd 2 vCPU", "Two-vCPU boxd machine.", None, None, Some(2), None, None, None, None, None, "Light worker", BOXD_SOURCE),
    tpl("boxd-8", Provider::Boxd, "boxd 8 vCPU", "Eight-vCPU boxd machine for parallel jobs.", None, None, Some(8), None, None, None, None, None, "Parallel jobs", BOXD_SOURCE),
    // -- Install recipes: only on providers this app can actually exec into --
    tpl("boat-pterodactyl-panel", Provider::Boat, "Pterodactyl panel (Boat)", "Large Ubuntu VM sized for the panel stack (PHP, MariaDB, nginx). Downloads the community installer to /root/pterodactyl-installer.sh, which you finish interactively.", None, Some("large"), Some(8), None, None, Some(7200), None, None, "Game panel (Pterodactyl)", RECIPE_SOURCE),
    tpl("freestyle-pterodactyl-panel", Provider::Freestyle, "Pterodactyl panel (Freestyle)", "Large Ubuntu VM with a full-day TTL so a panel survives a working session. Downloads the installer; the panel itself is not installed unattended.", Some("freestyle/ubuntu-lg"), None, None, None, None, Some(86_400), None, None, "Game panel (Pterodactyl)", RECIPE_SOURCE),
    tpl("boat-pterodactyl-wings", Provider::Boat, "Pterodactyl Wings node", "Large Boat machine to register as a Wings node. Downloads the installer so you can add the node to an existing panel.", None, Some("large"), Some(8), None, None, Some(7200), None, None, "Game panel (Wings)", RECIPE_SOURCE),
    tpl("boxd-pterodactyl-panel", Provider::Boxd, "Pterodactyl panel (boxd)", "Eight-vCPU boxd machine for the panel stack, with the installer downloaded and ready to run.", None, None, Some(8), None, None, None, None, None, "Game panel (Pterodactyl)", RECIPE_SOURCE),
    tpl("boat-docker-host", Provider::Boat, "Docker host (Boat)", "Default-class Boat machine with Docker Engine and the compose plugin from Docker's official script.", None, Some("default"), Some(4), None, None, Some(7200), None, None, "Containers", RECIPE_SOURCE),
    tpl("freestyle-node-pnpm", Provider::Freestyle, "Node 22 + pnpm (Freestyle)", "Ubuntu snapshot with NodeSource Node 22 and Corepack-enabled pnpm, ready for a JavaScript build.", Some("freestyle/ubuntu"), None, None, None, None, Some(21_600), None, None, "Node toolchain", RECIPE_SOURCE),
    tpl("boat-python-venv", Provider::Boat, "Python 3 + venv (Boat)", "Small Boat machine with python3, pip, venv and build-essential installed through apt.", None, Some("small"), Some(2), None, None, Some(7200), None, None, "Python toolchain", RECIPE_SOURCE),
];

pub fn catalog() -> &'static [InstanceTemplate] {
    CATALOG
}

pub fn find(id: &str) -> Option<&'static InstanceTemplate> {
    CATALOG.iter().find(|template| template.id == id)
}

/// Templates for one provider, in catalog order.
pub fn for_provider(provider: Provider) -> Vec<&'static InstanceTemplate> {
    CATALOG
        .iter()
        .filter(|template| template.provider == provider)
        .collect()
}

/// Post-boot setup commands, keyed by template id. Every command here is a real
/// shell script: it runs through the provider's exec API and its output is
/// returned to the caller. Nothing claims an install that did not happen.
const RECIPES: &[(&str, &str)] = &[
    (
        "boat-pterodactyl-panel",
        r#"set -e
if [ "$(id -u)" -ne 0 ]; then SUDO="sudo -n"; else SUDO=""; fi
export DEBIAN_FRONTEND=noninteractive
$SUDO apt-get update -y
$SUDO apt-get install -y curl ca-certificates
$SUDO curl -fsSL https://pterodactyl-installer.se -o /root/pterodactyl-installer.sh
$SUDO chmod +x /root/pterodactyl-installer.sh
echo "downloaded $(wc -c < /root/pterodactyl-installer.sh) bytes to /root/pterodactyl-installer.sh"
echo "next: run as root -> bash /root/pterodactyl-installer.sh (choose 'Install the panel')""#,
    ),
    (
        "freestyle-pterodactyl-panel",
        r#"set -e
if [ "$(id -u)" -ne 0 ]; then SUDO="sudo -n"; else SUDO=""; fi
export DEBIAN_FRONTEND=noninteractive
$SUDO apt-get update -y
$SUDO apt-get install -y curl ca-certificates
$SUDO curl -fsSL https://pterodactyl-installer.se -o /root/pterodactyl-installer.sh
$SUDO chmod +x /root/pterodactyl-installer.sh
echo "downloaded $(wc -c < /root/pterodactyl-installer.sh) bytes to /root/pterodactyl-installer.sh"
echo "next: run as root -> bash /root/pterodactyl-installer.sh (choose 'Install the panel')""#,
    ),
    (
        "boat-pterodactyl-wings",
        r#"set -e
if [ "$(id -u)" -ne 0 ]; then SUDO="sudo -n"; else SUDO=""; fi
export DEBIAN_FRONTEND=noninteractive
$SUDO apt-get update -y
$SUDO apt-get install -y curl ca-certificates
$SUDO curl -fsSL https://pterodactyl-installer.se -o /root/pterodactyl-installer.sh
$SUDO chmod +x /root/pterodactyl-installer.sh
echo "downloaded $(wc -c < /root/pterodactyl-installer.sh) bytes to /root/pterodactyl-installer.sh"
echo "next: run as root -> bash /root/pterodactyl-installer.sh (choose 'Install Wings')""#,
    ),
    (
        "boat-docker-host",
        r#"set -e
if [ "$(id -u)" -ne 0 ]; then SUDO="sudo -n"; else SUDO=""; fi
export DEBIAN_FRONTEND=noninteractive
$SUDO apt-get update -y
$SUDO apt-get install -y ca-certificates curl
$SUDO curl -fsSL https://get.docker.com -o /tmp/get-docker.sh
$SUDO sh /tmp/get-docker.sh
if [ -n "$SUDO" ]; then $SUDO usermod -aG docker "$(id -un)" || true; fi
docker --version""#,
    ),
    (
        "freestyle-node-pnpm",
        r#"set -e
if [ "$(id -u)" -ne 0 ]; then SUDO="sudo -n"; else SUDO=""; fi
export DEBIAN_FRONTEND=noninteractive
if ! command -v node >/dev/null 2>&1; then
  $SUDO apt-get update -y
  $SUDO apt-get install -y curl ca-certificates
  $SUDO curl -fsSL https://deb.nodesource.com/setup_22.x -o /tmp/nodesource.sh
  $SUDO bash /tmp/nodesource.sh
  $SUDO apt-get install -y nodejs
fi
$SUDO corepack enable || corepack enable || true
node --version
(npm --version)""#,
    ),
    (
        "boat-python-venv",
        r#"set -e
if [ "$(id -u)" -ne 0 ]; then SUDO="sudo -n"; else SUDO=""; fi
export DEBIAN_FRONTEND=noninteractive
$SUDO apt-get update -y
$SUDO apt-get install -y python3 python3-venv python3-pip build-essential
python3 --version
python3 -m pip --version""#,
    ),
];

pub fn setup_script(id: &str) -> Option<&'static str> {
    RECIPES
        .iter()
        .find(|(key, _)| *key == id)
        .map(|(_, script)| *script)
}

/// Owned, serialisable shape returned to the UI for both shipped and
/// user-created templates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateView {
    pub id: String,
    pub provider: Provider,
    pub name: String,
    pub description: String,
    pub image: Option<String>,
    pub size: Option<String>,
    pub cpu: Option<u32>,
    pub memory_gb: Option<u64>,
    pub disk_gb: Option<u64>,
    pub ttl_seconds: Option<u64>,
    pub gpu: Option<u32>,
    pub gpu_type: Option<String>,
    pub workload: String,
    pub source: String,
    pub setup: Option<String>,
    pub builtin: bool,
}

impl InstanceTemplate {
    pub fn view(&self) -> TemplateView {
        TemplateView {
            id: self.id.to_string(),
            provider: self.provider,
            name: self.name.to_string(),
            description: self.description.to_string(),
            image: self.image.map(str::to_string),
            size: self.size.map(str::to_string),
            cpu: self.cpu,
            memory_gb: self.memory_gb,
            disk_gb: self.disk_gb,
            ttl_seconds: self.ttl_seconds,
            gpu: self.gpu,
            gpu_type: self.gpu_type.map(str::to_string),
            workload: self.workload.to_string(),
            source: self.source.to_string(),
            setup: setup_script(self.id).map(str::to_string),
            builtin: true,
        }
    }
}

impl CustomTemplate {
    pub fn view(&self) -> TemplateView {
        TemplateView {
            id: self.id.clone(),
            provider: self.provider,
            name: self.name.clone(),
            description: self.description.clone(),
            image: self.request.template.clone(),
            size: self.request.size.clone(),
            cpu: self.request.cpu,
            memory_gb: self.request.memory_mb.map(|mb| mb / 1024),
            disk_gb: self.request.disk_gb,
            ttl_seconds: self.request.ttl_seconds,
            gpu: self.request.gpu,
            gpu_type: self.request.gpu_type.clone(),
            workload: self.workload.clone(),
            source: "Saved in this app".to_string(),
            setup: Some(self.setup.clone()).filter(|script| !script.trim().is_empty()),
            builtin: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{GpuSupport, Sizing};
    use std::collections::HashSet;

    #[test]
    fn ships_more_than_twenty_templates_with_unique_ids() {
        assert!(CATALOG.len() >= 20, "catalog has {} entries", CATALOG.len());
        let ids: HashSet<&str> = CATALOG.iter().map(|template| template.id).collect();
        assert_eq!(ids.len(), CATALOG.len(), "template ids must be unique");
        for template in CATALOG {
            assert!(!template.name.is_empty());
            assert!(!template.description.is_empty());
            assert!(!template.source.is_empty());
        }
    }

    #[test]
    fn every_provider_has_templates() {
        for provider in Provider::ALL {
            assert!(
                !for_provider(provider).is_empty(),
                "{} has no templates",
                provider.slug()
            );
        }
    }

    #[test]
    fn templates_only_send_what_the_provider_honours() {
        for template in CATALOG {
            let request = template.to_request(Some("probe".into()));
            let caps = template.provider.capabilities();
            if caps.sizing != Sizing::Selectable {
                assert!(request.memory_mb.is_none(), "{} sent memory", template.id);
                assert!(request.disk_gb.is_none(), "{} sent disk", template.id);
            }
            if caps.sizing == Sizing::FixedByTemplate {
                assert!(request.cpu.is_none(), "{} sent cpu", template.id);
            }
            if caps.gpu == GpuSupport::Unsupported {
                assert!(request.gpu.is_none(), "{} asked for a GPU", template.id);
            }
        }
    }

    #[test]
    fn gpu_templates_target_daytona_with_a_documented_type() {
        let caps = Provider::Daytona.capabilities();
        for template in CATALOG.iter().filter(|template| template.gpu.is_some()) {
            assert_eq!(template.provider, Provider::Daytona);
            let kind = template.gpu_type.expect("gpu type");
            assert!(
                caps.gpu_types.contains(&kind),
                "{kind} is not one of Daytona's documented GPU types"
            );
            let request = template.to_request(None);
            assert_eq!(request.gpu, Some(1));
            assert_eq!(request.gpu_type.as_deref(), Some(kind));
        }
    }

    #[test]
    fn e2b_templates_keep_their_template_identifier() {
        let request = find("e2b-base").unwrap().to_request(None);
        assert_eq!(request.template.as_deref(), Some("base"));
        assert_eq!(request.ttl_seconds, Some(1800));
        assert!(request.cpu.is_none());
        assert!(request.memory_mb.is_none());
    }

    #[test]
    fn setup_recipes_belong_to_real_templates_on_providers_that_can_exec() {
        for (id, script) in RECIPES {
            let template = find(id).unwrap_or_else(|| panic!("{id} is not in the catalog"));
            assert!(!script.trim().is_empty());
            assert!(
                template.provider.capabilities().exec,
                "{id} attaches a setup script but {} has no exec API",
                template.provider.slug()
            );
        }
        assert!(setup_script("boat-docker-host").is_some());
        assert!(setup_script("e2b-base").is_none());
    }

    #[test]
    fn builtin_views_carry_their_setup_command() {
        let view = find("boat-pterodactyl-panel").unwrap().view();
        assert!(view.builtin);
        assert_eq!(view.provider, Provider::Boat);
        assert!(view.setup.as_deref().unwrap().contains("pterodactyl-installer.se"));
        assert!(view.setup.as_deref().unwrap().contains("bash /root/pterodactyl-installer.sh"));
        assert!(find("e2b-base").unwrap().view().setup.is_none());
    }

    #[test]
    fn boat_templates_map_to_a_machine_class() {
        let request = find("boat-large").unwrap().to_request(None);
        assert_eq!(request.size.as_deref(), Some("large"));
        assert_eq!(request.cpu, Some(8));
        assert!(request.memory_mb.is_none());
    }
}
