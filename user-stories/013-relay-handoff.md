# Relay Handoff — Message Assignment

## Story
As a team lead, I want to hand a hurdle to a specific runner like a relay baton, so that ownership is clear and no two runners take the same jump.

## Requirements

### Requirement: Baton ownership
The system SHALL allow each hurdle to be assigned to exactly one runner at a time, SHALL show the assignee's avatar on the hurdle, and SHALL give each runner a "my lane" view containing only their hurdles.

#### Scenario: Lead passes the baton
- **WHEN** a lead assigns a hurdle to a runner
- **THEN** the runner's avatar appears on the hurdle and it enters their lane
- **AND** the runner gets an in-game notification ("Baton incoming!")

#### Scenario: Handoff mid-run
- **WHEN** an assigned hurdle is reassigned
- **THEN** it leaves the previous runner's lane and appears in the new runner's lane, keeping all message data and drafts

#### Scenario: Claiming a hurdle
- **WHEN** an unassigned hurdle is tapped and claimed by a runner
- **THEN** they become its owner, and other runners see it as taken
