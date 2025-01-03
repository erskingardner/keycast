# Keycast

## API

- [X] Move authentication checks to the api layer, not the model layer
- [X] Rationalize all the teams methods
- [X] DRY the error handling in api/http/teams.rs and the rest of the API
- [ ] Add audit logs

## Web

- [X] New policy form
- [X] Permissions form
- [X] policy card (with remove button)
- [ ] Remove key from team
- [ ] Remove authentication from key page
- [ ] Remove policies from team
- [ ] Audit logs

## Core

## Signer
- [X] Run signer as separate process (that is run by concurrently as well)
- [X] Finish permissions loop for approval

## Config
- [X] Make sure we reationalize config across all crates

## Deployments
- [ ] Docker
- [ ] Deploy to Umbrel
- [ ] Deploy to StartOS
