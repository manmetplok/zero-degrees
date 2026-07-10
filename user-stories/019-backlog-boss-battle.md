# Boss Battle — The Backlog Monster

## Story
As a team, we want the end of the course guarded by a Backlog Monster whose health is the open queue, so that beating it down together turns a bad Monday into a raid we win as a team.

## Requirements

### Requirement: Boss battle mode
The system SHALL represent the open backlog as a boss whose health corresponds to the weighted priority of open messages, SHALL reduce boss health with an attributed "hit" animation whenever any runner clears a hurdle, SHALL grow the boss when new messages arrive, and SHALL trigger a team-wide victory celebration when the backlog reaches zero.

#### Scenario: Every clear is a hit
- **WHEN** any team member clears a hurdle
- **THEN** the boss's health bar drops proportionally and the hit is credited on screen to that runner

#### Scenario: Boss enrage
- **WHEN** the number of burning (overdue) hurdles crosses a threshold
- **THEN** the boss enters an enrage state with escalated visuals, signaling the team to swarm the queue

#### Scenario: Victory
- **WHEN** the last open message is resolved
- **THEN** a victory screen credits the whole team with per-runner contribution stats

#### Scenario: Boss respawns
- **WHEN** new messages arrive after a victory
- **THEN** a new, appropriately-sized boss spawns without losing historical stats
