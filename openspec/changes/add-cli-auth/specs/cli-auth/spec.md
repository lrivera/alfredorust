## ADDED Requirements

### Requirement: CLI stores TOTP secret for automatic login

The system SHALL provide an `spcli login` command that stores the user's email and TOTP secret locally so the CLI can generate TOTP codes automatically.

#### Scenario: CLI setup succeeds

- **WHEN** the user runs `spcli login` with a server URL, email, and TOTP secret
- **THEN** the CLI stores the email and TOTP secret in a user-scoped credential file
- **AND** generates a current TOTP code to create a normal server session
- **AND** stores the returned session cookie locally

#### Scenario: CLI setup fails without enumeration

- **WHEN** the user runs CLI login with an unknown email or invalid TOTP secret
- **THEN** the CLI reports an authentication failure
- **AND** the server response does not reveal whether the email exists
- **AND** the CLI does not write or update the local credential file

#### Scenario: TOTP secret is recoverable locally

- **WHEN** CLI login succeeds
- **THEN** the CLI stores the TOTP secret in a recoverable local form
- **AND** it does not store one-time TOTP codes

### Requirement: CLI reauthenticates transparently with generated TOTP codes

The system SHALL generate current TOTP codes locally and perform a fresh server login when the current session is missing or rejected.

#### Scenario: Expired session with stored TOTP secret

- **WHEN** the user runs a protected CLI command with an expired session and stored TOTP secret
- **THEN** the CLI generates a current TOTP code
- **AND** logs in again through the server login route
- **AND** retries the protected request with the fresh session cookie

#### Scenario: Reauthentication fails

- **WHEN** the CLI cannot generate a valid TOTP code or the server rejects the generated login
- **THEN** the CLI reports that login is required
- **AND** it does not retry indefinitely

### Requirement: CLI credential file is local and restricted

The system SHALL store the TOTP secret outside the repository in a user-scoped binary encrypted envelope with restrictive permissions where supported.

#### Scenario: Credential file is created

- **WHEN** CLI login succeeds
- **THEN** the credential envelope stores the server base URL, email, TOTP secret, selected company context, current session cookie, and login metadata
- **AND** the file is created outside the repository with restrictive permissions where supported
- **AND** opening the file in a text editor does not reveal plaintext credentials

#### Scenario: Keyring is available

- **WHEN** the operating system keyring is available
- **THEN** the CLI uses keyring-protected material to encrypt or unlock the credential envelope

#### Scenario: Keyring is unavailable

- **WHEN** the operating system keyring is unavailable
- **THEN** the CLI derives a local wrapping key from machine/user material, app salt, and server/user salt
- **AND** uses authenticated encryption for the credential envelope

#### Scenario: Credential file is stolen

- **WHEN** an attacker obtains the credential file
- **THEN** the stored TOTP secret must be treated as potentially compromised
- **AND** the user must rotate their TOTP secret server-side to invalidate the stolen credential

#### Scenario: Credential envelope cannot be decrypted

- **WHEN** the CLI cannot decrypt the credential envelope
- **THEN** it fails loudly with instructions to run full login/setup again
- **AND** it does not delete the file automatically

### Requirement: CLI session storage is local and scoped

The system SHALL store CLI authentication state outside the repository in a user-scoped file containing only the information needed for subsequent server requests and automatic re-login.

#### Scenario: Session file is created after login

- **WHEN** CLI login succeeds
- **THEN** the session file stores the server base URL, tenant host context, session cookie, and login metadata
- **AND** the credential file stores the TOTP secret separately or in the same user-scoped config location
- **AND** it does not store one-time TOTP codes, passwords, certificate material, or production secrets

#### Scenario: Session file is unavailable

- **WHEN** the user runs a protected CLI command without a session file
- **THEN** the CLI fails loudly with instructions to run login

### Requirement: CLI status validates the stored session with the server

The system SHALL provide a status command that verifies whether the stored CLI session is accepted by the server and reauthenticates with the stored TOTP secret when needed.

#### Scenario: Stored session is valid

- **WHEN** the user runs CLI status with a stored unexpired session
- **THEN** the CLI calls a protected session-aware endpoint
- **AND** displays the authenticated user and active company context returned by the server

#### Scenario: Stored session is expired but TOTP secret is valid

- **WHEN** the user runs CLI status with an expired or rejected session
- **THEN** the CLI generates a current TOTP code and logs in again
- **AND** displays the authenticated user and active company context returned by the server

#### Scenario: Stored session and generated login are rejected

- **WHEN** the user runs CLI status with an expired session and a rejected generated login
- **THEN** the CLI reports that login is required

### Requirement: CLI requests preserve selected company context

The system SHALL send protected CLI requests with tenant host context derived from the selected company slug and configured base URL so server-side tenant selection remains authoritative.

#### Scenario: Tenant host is configured

- **WHEN** the user runs a protected CLI command after login
- **THEN** the CLI sends the stored session cookie
- **AND** sends the tenant host context for the selected company used by the server to select the active company

#### Scenario: Company context is missing

- **WHEN** the user attempts protected CLI usage before selecting a company
- **THEN** the CLI rejects the command before making the request
- **AND** instructs the user to run `spcli company list` and `spcli company use <slug>`

### Requirement: CLI can list and switch active company

The system SHALL allow an authenticated CLI user to list accessible companies and switch the active company context used by future commands.

#### Scenario: List accessible companies

- **WHEN** the user runs `spcli company list` with a valid stored session
- **THEN** the CLI retrieves the companies available to the authenticated user
- **AND** displays each company with enough information to select it for future commands

#### Scenario: Switch active company

- **WHEN** the user runs `spcli company use <slug>` for a company they belong to
- **THEN** the CLI updates the stored tenant host context for future protected requests
- **AND** future commands use the selected company context

#### Scenario: Switch to unavailable company

- **WHEN** the user attempts to select a company they do not belong to
- **THEN** the CLI rejects the selection or the server rejects the request
- **AND** the previous stored company context remains unchanged

### Requirement: CLI logout removes local authentication state

The system SHALL provide a logout command that removes local CLI session state and can optionally remove the stored TOTP secret.

#### Scenario: Logout with stored session

- **WHEN** the user runs CLI logout with a stored session
- **THEN** the CLI removes the local session cookie
- **AND** future protected CLI commands require login again

#### Scenario: Full logout removes stored secret

- **WHEN** the user runs full CLI logout
- **THEN** the CLI removes the local session cookie and stored TOTP secret
- **AND** future CLI usage requires configuring the TOTP secret again

#### Scenario: Logout without stored session

- **WHEN** the user runs CLI logout without a stored session
- **THEN** the CLI completes without creating session state
