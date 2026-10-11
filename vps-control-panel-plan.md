# VPS Control Panel — Product Plan & Architecture

> Working specification consolidated from the initial product discussion. This is a living plan, not a claim that every implementation detail has been finalised.

## 1. Product vision

Build a modular, native-first VPS control panel that lets people administer a Linux server through a web interface instead of needing SSH for every task.

The long-term goal is a **full VPS operating layer**: server administration, websites, application deployment, runtimes, services, scheduled jobs, databases, security settings, monitoring, installable modules and AI-assisted operations.

The panel itself must be independent of the application stacks it manages. The panel could be implemented in Laravel, Python, Java, .NET or another suitable stack; the applications deployed through it can use PHP, Python, Java, .NET, Node.js, Go, static files or other supported runtimes.

### Product principles

- Native-first: manage services and applications directly on Linux as the initial deployment model.
- Stack-agnostic: the control panel does not dictate the language of hosted applications.
- Modular: runtimes and additional capabilities can be added as modules.
- Safe by default: privileged operations are controlled, validated, logged and testable.
- Repeatable operations: jobs should survive browser disconnects and produce durable logs/status.
- AI-assisted, not AI-unrestricted: AI proposes or requests approved operations; it does not receive unrestricted root shell access.
- Simple installation and upgrades: aim for a one-command installer backed by versioned, verifiable Linux packages.
- Useful without AI: core server management should work even when the user has no AI API key.

## 2. Target users and positioning

### Target users

- Developers who can build applications but do not want to hand-configure Linux services and web servers.
- VPS owners who want a web interface for everyday server administration.
- People building applications with AI coding tools who need a straightforward way to deploy and operate them.
- Advanced users who want to extend the panel with modules rather than manage every tool separately.

### Positioning

Avoid positioning the product as merely another generic cPanel clone. The proposed angle is:

> A modular control plane for managing an entire VPS, with native application deployment and AI-assisted administration.

Competing products already cover parts of this market, including traditional hosting panels and deployment-first platforms. The product needs to differentiate through the coherent combination of whole-server management, native services, extensible modules and reliable guided operations.

**Product name:** not decided.

## 3. Architecture decisions

### 3.1 Logical components

1. **Web control panel**
   - UI, authentication, authorisation, configuration and public/internal APIs.
   - Application inventory, task history, audit trail and AI integration.
   - Does not run as unrestricted root.

2. **Rust privileged agent**
   - A separate operating-system service responsible for approved privileged actions.
   - Examples include managing panel-owned services, installing approved packages, writing panel-owned configuration files and validating/reloading Nginx.
   - Exposes a small, explicitly defined operation interface, not a generic `execute_root_command` interface.

3. **Task/job layer**
   - Persists task requests and statuses before execution.
   - Captures progress, output, errors, timestamps and outcomes.
   - Allows users to reload the page or disconnect while work continues.
   - Supports retries only when an operation is safe to retry.

4. **Runtime and feature modules**
   - Implement stack-specific or optional capabilities on top of shared interfaces.
   - Initial modules should be first-party and versioned; a public third-party marketplace comes later.

5. **Linux services**
   - Use mature native tools such as systemd, APT, Nginx, PHP-FPM and database servers instead of trying to reimplement them.

### 3.2 Panel language is not decided

Python was the initial idea, but the panel does **not** have to be written in Python. Choose the web stack that enables safe, rapid development and long-term maintainability.

Candidates discussed:

- **Laravel + HTMX + Bootstrap 5**: attractive because it matches existing project familiarity.
- **Python + FastAPI + Jinja2 + HTMX + Bootstrap 5**: a viable alternative for a Python-first panel.

The UI can be server-rendered; a separate JavaScript SPA is not required for the initial product.

### 3.3 Agent language

**Decision direction: Rust for the privileged agent.**

Rationale:

- Produces a compiled native executable and generally has low runtime overhead.
- Provides strong compile-time memory-safety guarantees when using safe Rust.
- Fits a small, long-running system service at a privilege boundary.

Trade-off: Rust introduces a learning curve and may initially slow implementation. Performance is not expected to be the primary bottleneck for operations such as package installation and service management; reliability and security matter more.

### 3.4 One repository, two processes

Start with **one GitHub monorepo** and **two separately running applications**:

- Control panel
- Rust agent

A single repository simplifies AI-assisted development, shared changes, integration tests and coordinated releases. Revisit splitting repositories only when separate teams or genuinely independent release schedules justify it.

Keep the components independently buildable and testable from the start.

### 3.5 Communication and privilege boundary

- Prefer a local Unix-domain socket with restrictive ownership and permissions for panel-to-agent communication.
- Use a versioned protocol with structured requests and responses.
- The panel authenticates/authorises user requests; the agent independently validates operation names, parameters, paths and allowed resource namespaces.
- Consider systemd D-Bus for appropriate service-management operations.
- Avoid shell-string execution. Use argument arrays, timeouts, bounded output and strict validation when external tools are necessary.
- Do not expose an operation that accepts arbitrary commands and executes them as root.
- Use the minimum necessary privileges for each action and log all privileged mutations.

Illustrative request only:

```json
{
  "protocol_version": 1,
  "operation": "service.restart",
  "parameters": {
    "service_id": "managed-app-123"
  }
}
```

The agent must resolve `service_id` through its managed-resource registry and must not trust an arbitrary service name supplied by a caller.

### 3.6 Protocol versioning

The panel and agent should negotiate or report protocol versions and supported capabilities. Version the communication contract independently from internal implementation details. Add contract tests so changes do not silently break compatibility.

## 4. Proposed repository layout

The exact framework folders will depend on the panel stack chosen later. A starting structure could be:

```text
vps-control/
├── panel/                  # Web application (Laravel or Python, decision pending)
├── agent/                  # Rust Cargo project
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs
│   │   ├── operations/
│   │   ├── security/
│   │   └── protocol/
│   └── tests/
├── contracts/              # Versioned API schemas and operation definitions
├── modules/                # First-party module manifests/definitions
├── packaging/              # systemd units, Debian packaging and installer
├── tests/
│   └── integration/
├── docs/
├── .github/workflows/      # CI and release automation
└── README.md
```

`panel/` should include its UI/templates as part of the same web application. There is no requirement for a separate frontend repository for HTMX/server-rendered UI.

## 5. Supported deployment stacks

The control panel's own language is independent of hosted applications. Plan a common application model with runtime-specific providers/modules.

| Runtime / application type | Responsibilities of its provider/module |
|---|---|
| PHP / Laravel | PHP version, PHP-FPM pool, Composer dependencies, Nginx site, optional database |
| Python / Django / Flask / FastAPI | Python runtime, virtual environment, dependencies, systemd service, Nginx upstream |
| Java / Spring Boot | JDK version, JAR deployment, JVM settings, systemd service, Nginx upstream |
| .NET / ASP.NET Core | Supported .NET runtime, published app, Kestrel/systemd configuration, Nginx upstream |
| Node.js | Node version, package manager, build/start commands, service, Nginx upstream |
| Go | Build or binary deployment, environment and service configuration |
| Static websites | Website files and Nginx configuration |
| Generic scripts/workers | Defined command, user, working directory, environment and restart policy |
| Scheduled tasks | Cron or systemd timer management, execution history and error output |

Compatibility still depends on OS version, CPU architecture, dependencies, runtime support and available memory/disk. Legacy .NET Framework applications that require Windows are outside the native Linux scope.

### Runtime provider contract

Each provider should implement a common conceptual lifecycle, including:

- Detect/validate project type and prerequisites.
- Declare supported OS/runtime versions.
- Prepare runtime/dependencies.
- Generate application configuration.
- Deploy/start/restart/stop the application.
- Run a health check.
- Collect status/log references.
- Support rollback where the operation is safely reversible.

The generic deployment path should also allow a user to define build/start commands and health checks. Those commands must run as the application's own Linux user, not root.

## 6. Native application deployment design

A managed application should have a durable identity and configuration, including:

- ID and display name.
- Runtime/provider and runtime version.
- Linux owner/user and filesystem paths.
- Source and deployment method.
- Build and start configuration, if relevant.
- Environment variables/secrets.
- Domains and web-server routing.
- Service definition and background workers.
- Health-check configuration.
- Persistent data and uploads.
- Deployment history, task history and rollback records.

### Suggested release-directory pattern

For applicable applications, deploy releases into separate directories and switch a `current` symlink only after the new release passes validation/health checks.

```text
/opt/vps-panel/apps/myapp/
├── releases/
│   ├── <release-id-1>/
│   └── <release-id-2>/
├── shared/
│   ├── uploads/
│   └── logs/
└── current -> releases/<active-release-id>
```

The layout is illustrative and should be configurable. Persistent user data must be outside ephemeral release directories.

### Deployment lifecycle

1. Fetch or upload source.
2. Create a new release directory.
3. Prepare runtime and dependencies.
4. Generate/validate configuration.
5. Start or stage the new application under its own Linux user.
6. Run health checks.
7. Switch traffic only after successful validation.
8. Keep a previous known-good release where applicable.
9. Record the result and make logs accessible in the panel.

Nginx configuration should be generated only for panel-managed sites. Test the configuration before reload, retain a previous working configuration, and define recovery behaviour if a reload or health check fails.

## 7. Module system

### Core principle

The core should support installable capability modules without requiring changes throughout the codebase every time a runtime is added.

Possible future modules include:

- Python runtime
- PHP/PHP-FPM runtime
- Node.js runtime
- Java runtime
- .NET runtime
- Database management
- Backup providers
- Monitoring
- Firewall management
- DNS integrations
- Additional web-server or application integrations

### Module manifest

A manifest should identify a module's name, version, supported OS versions, capabilities, dependencies and operation handlers. Example only:

```yaml
id: python-runtime
name: Python Runtime Manager
version: 0.1.0
platforms:
  - ubuntu-24.04
capabilities:
  - runtime.manage
  - application.deploy
operations:
  - install_runtime
  - create_virtualenv
  - deploy_application
```

This is a proposed format, not an established standard.

### Module safety

- Start with first-party modules bundled with or distributed by the project.
- Review module code and installation scripts before publishing.
- Declare permissions/capabilities explicitly and show consequential changes before execution.
- Track versions, dependencies, installation state and upgrades.
- Provide uninstall plans and preserve user data unless deletion is explicitly requested.
- Treat module manifests as metadata, **not as a sandbox**. Arbitrary third-party code running as root can compromise the host.
- Introduce a public third-party marketplace only after the security model and module lifecycle mature.

## 8. MVP scope

The first release should solve a single reliable end-to-end workflow before attempting to implement every feature found in cPanel.

### MVP features

1. **Installer and bootstrap**
   - Detect supported OS and architecture.
   - Install panel and agent.
   - Initialise database and services.
   - Provide first-run administrator setup with no hardcoded default password.

2. **Authentication and audit**
   - Secure admin login.
   - MFA should be included before broad public exposure; ideally establish its architecture from the beginning.
   - Session security, audit events and operation records.

3. **Server overview**
   - CPU, RAM, disk, uptime, OS information and managed service status.

4. **First native application workflow**
   - Deploy a Python web application.
   - Create a dedicated Linux user and virtual environment.
   - Create/manage a systemd service.
   - Configure an Nginx site and domain.
   - Inspect logs, restart, check health and roll back a supported deployment.

5. **Web-server and TLS management**
   - Generate panel-owned Nginx configurations.
   - Validate configuration before reload.
   - Support automatic TLS issuance/renewal through a provider abstraction and allow user-supplied certificates.

6. **Basic OS administration**
   - View service status and control panel-managed services.
   - Install approved packages from configured OS repositories.
   - View firewall status initially; avoid broad destructive firewall changes until recovery safeguards exist.
   - Manage basic scheduled jobs.

7. **Durable job handling**
   - Persist tasks before running them.
   - Show progress/output/failures after a browser refresh.
   - Record actor, operation, target resource and result.

8. **Recovery basics**
   - Back up panel configuration and data.
   - Define a tested restore path.
   - Provide rollback for supported deployments/configuration changes.

### Add after the first vertical slice

- PHP/PHP-FPM and common Laravel workflow.
- Database provisioning and management.
- Git-based deployments and more generic runtime definitions.
- More extensive monitoring and backups.
- Java, .NET and Node.js providers.

### Explicitly defer

- Full email hosting and spam/deliverability management.
- General-purpose browser root terminal.
- Unrestricted public plugin marketplace.
- Support for every Linux distribution.
- Multi-server orchestration and clusters.
- Fully autonomous AI changes without review/guardrails.

## 9. AI strategy

AI is an assistant for operating the control plane, not the privilege boundary itself.

### Good initial AI workflows

- Explain service/application errors from actual logs and status.
- Suggest diagnostic checks.
- Draft a deployment plan from a natural-language request.
- Explain what a configuration change will do.
- Propose remediation and call approved operations after the required approval.

### Example troubleshooting flow

1. Resolve which managed application the user means.
2. Query its status and relevant logs through authorised tools.
3. Form a diagnosis grounded in the collected evidence.
4. Propose a change, explain impact and obtain approval when consequential.
5. Invoke a structured operation through the normal validation path.
6. Verify health and report the result.

### AI guardrails

- No unrestricted AI-generated root shell commands.
- Use provider-neutral adapters so the model provider can change.
- Make API use opt-in/configurable and set spending limits.
- Do not send secrets or unnecessary log contents to an external provider.
- Keep a useful non-AI mode.
- Log requested action, approval, operation and verification result.

## 10. Security requirements

Because the panel administers the whole server, security is a core product capability.

- Web process runs unprivileged.
- Rust agent exposes a small allowlisted operation API.
- Agent validates resource identity and parameters independently of the panel.
- Never accept arbitrary root commands from the web UI or AI.
- Restrict filesystem paths to explicit managed locations where possible; defend against symlinks and path traversal.
- Use secure secret storage and avoid writing secrets to logs.
- Protect the agent's socket with correct ownership and permissions.
- Apply authentication, CSRF protections where relevant, secure sessions and MFA.
- Keep audit records for privileged changes.
- Make generated configuration changes transactional where practical.
- Set timeouts and output limits for child processes.
- Avoid destructive firewall operations that can lock out the user without an explicit recovery mechanism.
- Test cross-application isolation: one application's Linux user must not read another application's secrets.
- Provide documented recovery and restore steps.
- Run security tests against the panel-to-agent boundary and every privileged operation.

## 11. Operating-system support

**Initial target:** Ubuntu 24.04 LTS on `amd64` as the first validated platform.

After automated tests are in place, evaluate Ubuntu 26.04 LTS and `arm64`, and only advertise them once installation, upgrades, runtime modules and recovery paths have been tested. Keep an explicit support matrix rather than claiming compatibility with all Linux distributions.

Use a clean disposable VM for destructive install/upgrade tests and a separate development environment.

## 12. Release and distribution strategy

### User experience goal

Eventually, a supported user can run a single installer command such as:

```bash
curl -fsSL https://get.example.com/install.sh | sudo bash
```

`example.com` is a placeholder and must be replaced with the official product domain. Do not advertise this command until the installer and domain are actually available.

A `curl | sudo bash` command runs the downloaded script as root. Keep the bootstrap small and reviewable, use HTTPS, and ensure it installs verified artifacts rather than blindly cloning and executing the latest development branch.

### Release pipeline

Start from a GitHub monorepo and automate releases through CI:

1. Developer creates a version tag, such as `v0.1.0`.
2. CI runs unit, integration, protocol and security-related tests.
3. CI builds the Rust agent for supported architectures.
4. CI packages the panel and agent.
5. CI generates checksums and signs release artifacts/packages.
6. CI publishes versioned release assets.
7. Install and upgrade tests validate the built artifacts on clean VMs.

### Package design

Initially consider two Debian packages:

- `vpspanel-agent`: Rust binary, agent service, socket permissions and relevant policies.
- `vpspanel`: web application, runtime dependencies, service unit, migrations and panel configuration.

The panel package can depend on the agent package. Exact dependencies will depend on the panel framework chosen.

### Long-term installation and updates

- Use GitHub Releases for versioned builds/assets while the project is small.
- Establish a signed APT repository before wider adoption.
- Use native package management for routine updates instead of asking users to rerun the bootstrap installer for every release.
- Make package maintainer scripts idempotent and safe to retry.
- Back up critical state before upgrades, apply migrations carefully and verify health afterward.
- Document rollback/recovery limitations honestly.

Example update command once the signed repository is configured:

```bash
sudo apt update
sudo apt install --only-upgrade vpspanel
```

### Integrity and signing

Checksums detect accidental corruption but do not independently authenticate a release if the checksum and artifact are downloaded from the same compromised location. Use signed package repository metadata or verifiable artifact signatures, protect signing credentials and document verification.

### Service onboarding

For the first release, prefer not to expose an unauthenticated setup UI to the public internet. Provide a secure first-run setup and a documented local/SSH-tunnel approach while the administrator configures domain, TLS and firewall access.

## 13. Development roadmap

### Stage 1 — Foundations

- Decide panel stack and product name later, after initial technical validation.
- Establish monorepo, coding standards, CI and test environment.
- Implement authentication, database migrations, durable task model and audit trail.
- Build Rust agent skeleton and a versioned request/response protocol.
- Implement socket permissions and the first small set of allowlisted operations.

**Exit criterion:** a safe, predefined administrative operation can be requested and completed without giving the panel unrestricted root execution.

### Stage 2 — One complete native app deployment

- Create a Python application from source or a minimal project template.
- Provision the application's own Linux user and virtual environment.
- Create a systemd service through validated templates.
- Generate and test an Nginx configuration.
- Verify health, show logs, support restart and implement a supported rollback path.

**Exit criterion:** deploy a reachable application on a clean supported VPS entirely through the web panel.

### Stage 3 — Server administration

- Add server metrics, managed service controls, package operations and scheduled jobs.
- Add domains and TLS lifecycle.
- Implement basic backup/restore.
- Test failure handling and recovery.

### Stage 4 — Runtime modules

- Extract Python deployment capability into a first-class runtime provider/module.
- Add PHP/PHP-FPM, database provisioning and generic service support.
- Add further runtimes based on user demand and tested compatibility.

### Stage 5 — AI assistance

- Add evidence-based diagnostics and deployment plan generation.
- Invoke only approved structured operations.
- Require confirmation for consequential changes.
- Add provider configuration, quotas and redaction of sensitive data.

### Stage 6 — Ecosystem and scale

- Add independently versioned modules, a review/signing model and upgrade flow.
- Explore a central multi-server dashboard, team permissions and managed services only after the single-server experience is reliable.

## 14. Definition of done for the first public beta

The beta should not be considered ready until tests verify that:

- Installation works on a clean supported Ubuntu VM.
- Upgrade works without losing panel data or application configuration.
- A native app can be deployed, restarted, inspected and rolled back through the panel.
- Invalid Nginx configuration cannot silently replace the last working configuration.
- The web process cannot turn the agent into an arbitrary root command runner.
- The application user cannot access another application's secrets.
- A disconnected browser can reconnect and inspect a continuing task.
- Failed actions produce useful logs and a clear status.
- Backup restoration has been tested on a separate VM.
- Release artifacts and package metadata can be verified.
- The recovery procedure is documented and tested.

## 15. Open decisions

These remain deliberately undecided and should be resolved before implementation becomes large:

1. **Product name and domain.**
2. **Panel framework:** Laravel (a strong fit with current familiarity) versus Python/FastAPI, or another stack.
3. **Initial authentication/MFA approach.**
4. **TLS approach:** which supported providers and whether DNS-based challenges are needed initially.
5. **Application source workflows:** Git repositories, uploaded archives, server-side directories, or a minimal selection for beta.
6. **Database engines for early provisioning:** for example MariaDB/PostgreSQL, with a decision based on target users.
7. **Exact agent protocol and operation schemas.**
8. **Secret storage model and backup/encryption approach.**
9. **Release signing and package hosting setup.**
10. **First beta boundaries and whether Docker is explicitly excluded or available as an optional later module.** Native-first does not mean Docker can never be supported.

## 16. Initial implementation recommendation

Begin with one narrowly defined vertical slice rather than building a full dashboard first:

**Install panel → create a Python app → assign a Linux user → create virtual environment → create systemd service → configure Nginx → connect domain → inspect logs → restart or roll back.**

This will exercise the critical architecture: permissions, protocol design, durable jobs, configuration generation, failure handling and recovery. Once the workflow is safe and repeatable, add more runtimes and capabilities through the same shared interfaces.

## References for implementation

- GitHub Releases: <https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases>
- Debian maintainer scripts policy: <https://www.debian.org/doc/debian-policy/ch-maintainerscripts>
- Sigstore Cosign quickstart: <https://docs.sigstore.dev/quickstart/quickstart-cosign/>
- systemd D-Bus documentation: <https://wiki.freedesktop.org/www/Software/systemd/dbus/>
- polkit documentation: <https://polkit.pages.freedesktop.org/polkit/polkit.8.html>
- Nginx command-line switches and configuration testing: <https://nginx.org/en/docs/switches.html>
- FastAPI deployment concepts: <https://fastapi.tiangolo.com/deployment/concepts/>
- Ubuntu release cycle: <https://ubuntu.com/about/release-cycle>

---

**Status:** Product direction, native-first strategy, Rust-agent direction, monorepo approach, module concept, phased MVP and release approach are agreed directions. The product name and the panel's own framework remain open decisions.
