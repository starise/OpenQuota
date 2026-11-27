CREATE TABLE trajectory_meta (trajectory_id TEXT, cascade_id TEXT);
CREATE TABLE gen_metadata (idx INTEGER PRIMARY KEY, data BLOB);
CREATE TABLE steps (idx INTEGER PRIMARY KEY, metadata BLOB);
INSERT INTO trajectory_meta VALUES ('fixture-trajectory', 'fixture-conversation');

-- Replacement model names and dates, with counters from the CLI probe.
-- Row 0 has unknown Protobuf fields, a timestamp and aggregate/split output.
INSERT INTO gen_metadata VALUES (0, X'AD06000000009806040A249A010C7072696365642D6D6F64656C220910A7631840483B50054A08220608C0E0C8D206120101');
-- Row 1 refers to a missing step and then step 3. Step 1 has the wrong date.
INSERT INTO gen_metadata VALUES (1, X'0A399A011267656D696E692D70726F2D64656661756C74220C10A217182628A65348215005AA0113507269636564204D6F64656C2028486967682912026303');
INSERT INTO steps VALUES (1, X'0A0608C0BDC3D206'), (3, X'0A0608C0E0C8D206');

-- Split output only, with separate cache reads/writes and an unknown model.
INSERT INTO gen_metadata VALUES (2, X'0A279A010E756E7072696365642D6D6F64656C220A100A200B280C480D500E4A08220608C0E0C8D206');
-- Usage with unfamiliar field numbers and a separate truncated record.
INSERT INTO gen_metadata VALUES (3, X'0A249A0112696E636F6D70617469626C652D6D6F64656C2203E003034A08220608C0E0C8D206');
INSERT INTO gen_metadata VALUES (4, X'0AFF');
