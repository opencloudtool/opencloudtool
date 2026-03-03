# OCT-CLOUD Crate Context

This crate (`oct-cloud`) provides AWS infrastructure provisioning and management
through dependency graphs. It wraps AWS SDK clients, defines resource types, and
orchestrates creation/destruction order via topological sort.

## Architecture

- **Resource Trait** (`resource.rs`):
  - `Resource { create(), destroy() }` — basic async lifecycle interface.

- **Manager Trait** (`infra/resource.rs`):
  - `Manager<'a, I, O> { create(input, parents), destroy(input, parents) }` — generic resource
    manager that receives parent nodes for dependency context.
  - 11 resource types, each with a `*Spec` input and realized output struct:
    Vpc, InternetGateway, RouteTable, Subnet, SecurityGroup, InstanceRole,
    InstanceProfile, Vm, HostedZone, DnsRecord, Ecr.

- **Graph Model** (`infra/graph.rs`):
  - `petgraph::Graph<Node, String>` with `Node::Root` (synthetic) and `Node::Resource(ResourceType)`.
  - `GraphManager` initializes AWS SDK clients and exposes:
    - `deploy_genesis_graph()` — minimal bootstrap deployment for the leader node.
    - `kahn_traverse()` — topological sort respecting dependency edges.
  - This is the largest file in the crate (~73 KB); prefer targeted line-range reads.

- **State** (`infra/state.rs`):
  - `State::from_graph()` / `to_graph()` — serializable round-trip between `petgraph` and flat struct.
  - Resources sorted by dependency depth then alphabetically.
  - `get_vms()` extracts VM entries from state.

- **AWS Client Wrappers** (`aws/client.rs`):
  - `Ec2Impl`, `IAMImpl`, `ECRImpl`, `Route53Impl`, `S3Impl` — thin wrappers with `#[automock]`.
  - Type aliases (`pub use Ec2Impl as Ec2`) switch to mock variants under `#[cfg(test)]`.

- **AWS Types** (`aws/types.rs`):
  - `InstanceType` enum (T3 nano→2xlarge) with `from_resources(cpus, memory)` bin-packing.
  - `RecordType` enum (A, NS, SOA, TXT) with AWS SDK conversions.

## Testing

- **Run tests:**
  ```bash
  cargo test -p oct-cloud
  ```
- **Mock pattern:** `mockall` `#[automock]` on AWS client impl blocks; tests construct
  mock clients with `expect_*()` expectations.
- **Test locations:** inline `#[cfg(test)] mod tests` in `infra/state.rs`, `infra/resource.rs`,
  `aws/types.rs`, `aws/resource.rs`.
- **Style:** explicit `// Arrange`, `// Act`, `// Assert` sections.

## Symlinks

- Keep `CLAUDE.md` and `GEMINI.md` in this directory as symlinks to `AGENTS.md`.

## Directory Index

- `AGENTS.md` - Local crate-specific agent instructions.
- `CLAUDE.md` - Symlink to `AGENTS.md`.
- `GEMINI.md` - Symlink to `AGENTS.md`.
- `src/` - Rust source code for `oct-cloud`.
  - `lib.rs` - Module exports (`pub mod infra`, `pub mod resource`, `pub mod aws`).
  - `resource.rs` - `Resource` trait definition.
  - `aws/` - AWS SDK client wrappers and types.
  - `infra/` - Graph manager, resource managers, and state serialization.
