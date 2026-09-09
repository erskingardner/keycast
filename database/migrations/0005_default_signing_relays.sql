-- Current fresh-install defaults: nos.lol, Primal and Damus.
-- Keep applied migrations immutable and preserve routes already issued to clients.
-- Only replace the untouched bootstrap set before any grant has been created.
UPDATE relays SET url='wss://relay.damus.io'
WHERE url='wss://bucket.coracle.social' AND enabled=1 AND sort_order=30 AND discovered=0
AND NOT EXISTS(SELECT 1 FROM grants)
AND (SELECT count(*) FROM relays)=3
AND EXISTS(SELECT 1 FROM relays WHERE url='wss://nos.lol' AND enabled=1 AND sort_order=10 AND discovered=0)
AND EXISTS(SELECT 1 FROM relays WHERE url='wss://relay.primal.net' AND enabled=1 AND sort_order=20 AND discovered=0);
