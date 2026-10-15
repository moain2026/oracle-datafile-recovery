-- Block dumps (ALTER SYSTEM DUMP DATAFILE) of the segment-header block and the first
-- 3 data blocks of every corpus segment: the reference traces for lab-verify.
-- Trace files land in the diag trace dir with identifier ODR_DUMP; lab.sh copies them.
WHENEVER SQLERROR CONTINUE
SET SERVEROUTPUT ON
ALTER SESSION SET CONTAINER = &1;
ALTER SESSION SET TRACEFILE_IDENTIFIER = 'odr_dump';
BEGIN
  FOR s IN (SELECT e.file_id, e.block_id, e.blocks, e.segment_name FROM dba_extents e
            WHERE e.owner = 'ODR' AND e.extent_id = 0 ORDER BY e.segment_name) LOOP
    EXECUTE IMMEDIATE 'ALTER SYSTEM DUMP DATAFILE ' || s.file_id || ' BLOCK MIN ' || s.block_id ||
                      ' BLOCK MAX ' || (s.block_id + LEAST(s.blocks, 6) - 1);
    DBMS_OUTPUT.PUT_LINE('dumped ' || s.segment_name || ' ' || s.file_id || ' ' || s.block_id);
  END LOOP;
  -- File header block of each corpus datafile (kcvfh, LV-028) and bootstrap$ root (LV-025).
  FOR f IN (SELECT file_id FROM dba_data_files WHERE tablespace_name LIKE 'ODR%' OR tablespace_name = 'SYSTEM') LOOP
    EXECUTE IMMEDIATE 'ALTER SYSTEM DUMP DATAFILE ' || f.file_id || ' BLOCK 1';
  END LOOP;
END;
/
ALTER SESSION SET EVENTS 'immediate trace name file_hdrs level 10';
EXIT
