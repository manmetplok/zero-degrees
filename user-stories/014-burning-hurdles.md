# Burning Hurdles — Response-Time Pressure

## Story
As a team lead, I want hurdles to show a countdown and catch fire when they wait past their response target, so that slow replies become impossible to miss instead of silently slipping.

## Requirements

### Requirement: Countdown and overdue state
The system SHALL display each open hurdle's waiting time, SHALL support a configurable response-time target per urgency level, SHALL set hurdles visually "on fire" when the target is exceeded, and SHALL record response times of cleared hurdles for team statistics.

#### Scenario: Hurdle ignites
- **WHEN** an open message exceeds the response-time target for its urgency level
- **THEN** its hurdle starts burning on the track and is counted on race control

#### Scenario: Dousing the flames
- **WHEN** a burning hurdle is cleared
- **THEN** a "fire extinguished" effect plays and partial points are still awarded (less than an on-time clear)

#### Scenario: On-time clear
- **WHEN** a hurdle is cleared within its target
- **THEN** its response time is recorded and a speed bonus applies
