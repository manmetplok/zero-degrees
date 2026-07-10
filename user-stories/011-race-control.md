# Race Control — The Team Lead's Map

## Story
As a support team lead, I want a "race control" screen showing the whole course — hurdle counts, types, moods, hazard zones, and every runner's position — so that I finally have the overview nobody has today.

## Requirements

### Requirement: Race control dashboard
The system SHALL provide an overview screen showing at minimum: open vs. cleared hurdle counts, volume per channel, distribution per hurdle type, mood breakdown, active hazard zones, overdue (burning) hurdles, and per-runner progress.

#### Scenario: Lead checks the course
- **WHEN** a lead opens race control
- **THEN** live counts and distributions reflect the latest ingested and resolved messages

#### Scenario: Drill-down
- **WHEN** the lead taps any segment (e.g. the "complaint" hurdle type)
- **THEN** the track view opens filtered to the corresponding hurdles

#### Scenario: Demo-friendly
- **WHEN** race control is shown on a big screen during a live demo
- **THEN** hurdle clears by any runner animate in near-real-time
