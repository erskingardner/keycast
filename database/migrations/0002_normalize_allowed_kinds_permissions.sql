-- Normalize the old web permission shape before the hardened signer starts
-- rejecting unknown fields in allowed_kinds configs.
--
-- Old shape:
--   {"sign":[1,7],"encrypt":null,"decrypt":null}
--
-- Current shape:
--   {"allowed_kinds":[1,7]}

UPDATE permissions
SET config = json_object('allowed_kinds', json_extract(config, '$.sign'))
WHERE identifier = 'allowed_kinds'
  AND json_valid(config)
  AND json_type(config, '$.allowed_kinds') IS NULL
  AND json_type(config, '$.sign') IN ('array', 'null');
