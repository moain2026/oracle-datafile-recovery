-- Golden truth export (ADR-0004). Produced by Oracle itself, independent of ODR's
-- decoders. For every corpus table and every column it writes one line per value:
--   TABLE|ID|COLUMN|ROWID|DUMP(col,1016)|canonical text
-- DUMP(...,1016) gives "Typ=.. Len=..: <hex bytes>" plus the charset: the exact
-- bytes stored in the block, which is what ODR must reproduce. The canonical text
-- uses fixed NLS settings so every rebuild produces identical output.
-- Also writes: database/charset/version facts, DBA_SEGMENTS/DBA_EXTENTS (where each
-- table lives, block by block), datafile list, and row counts.
-- Usage (inside the container): sqlplus -s / as sysdba @20_export_golden.sql <PDB> <outdir>
WHENEVER SQLERROR EXIT FAILURE
SET FEEDBACK OFF HEADING OFF PAGESIZE 0 LINESIZE 32767 LONG 2000000 TRIMSPOOL ON TERMOUT OFF VERIFY OFF ECHO OFF
ALTER SESSION SET CONTAINER = &1;
ALTER SESSION SET NLS_DATE_FORMAT = 'SYYYY-MM-DD HH24:MI:SS';
ALTER SESSION SET NLS_TIMESTAMP_FORMAT = 'SYYYY-MM-DD HH24:MI:SS.FF9';
ALTER SESSION SET NLS_TIMESTAMP_TZ_FORMAT = 'SYYYY-MM-DD HH24:MI:SS.FF9 TZH:TZM';
ALTER SESSION SET NLS_NUMERIC_CHARACTERS = '.,';
ALTER SESSION SET TIME_ZONE = '+00:00';

SPOOL &2/meta/database.txt
SELECT 'version|' || banner_full FROM v$version;
SELECT 'param|' || name || '|' || value FROM v$parameter WHERE name IN ('compatible','db_block_size','db_name');
SELECT 'nls|' || parameter || '|' || value FROM nls_database_parameters ORDER BY parameter;
SELECT 'pdb|' || name || '|' || con_id || '|' || dbid FROM v$pdbs ORDER BY con_id;
SELECT 'tde|' || COUNT(*) FROM dba_tablespaces WHERE encrypted = 'YES';
SPOOL OFF

SPOOL &2/meta/datafiles.txt
SELECT file_id || '|' || relative_fno || '|' || tablespace_name || '|' || file_name || '|' || blocks || '|' ||
       (SELECT bigfile FROM dba_tablespaces t WHERE t.tablespace_name = f.tablespace_name)
FROM dba_data_files f ORDER BY file_id;
SPOOL OFF

SPOOL &2/meta/segments.txt
SELECT o.object_name || '|' || o.object_id || '|' || o.data_object_id || '|' || s.segment_type || '|' ||
       s.tablespace_name || '|' || s.header_file || '|' || s.header_block
FROM dba_objects o JOIN dba_segments s ON s.owner = o.owner AND s.segment_name = o.object_name
WHERE o.owner = 'ODR' AND o.object_type IN ('TABLE','CLUSTER') ORDER BY o.object_name;
SPOOL OFF

SPOOL &2/meta/extents.txt
SELECT segment_name || '|' || extent_id || '|' || file_id || '|' || block_id || '|' || blocks
FROM dba_extents WHERE owner = 'ODR' ORDER BY segment_name, extent_id;
SPOOL OFF

SPOOL &2/meta/columns.txt
SELECT table_name || '|' || column_id || '|' || column_name || '|' || data_type || '|' ||
       NVL(TO_CHAR(data_length),'') || '|' || NVL(TO_CHAR(data_precision),'') || '|' ||
       NVL(TO_CHAR(data_scale),'') || '|' || nullable || '|' || NVL(TO_CHAR(segment_column_id),'') || '|' ||
       hidden_column || '|' || virtual_column
FROM dba_tab_cols WHERE owner = 'ODR' ORDER BY table_name, internal_column_id;
SPOOL OFF

-- Row-by-row, column-by-column truth. Generated SQL keeps this script schema-agnostic.
SET SERVEROUTPUT ON SIZE UNLIMITED
SPOOL &2/rows.psv
DECLARE
  v_sql   CLOB;
  v_cur   SYS_REFCURSOR;
  v_line  VARCHAR2(32767);
BEGIN
  FOR t IN (SELECT table_name FROM dba_tables WHERE owner = 'ODR' ORDER BY table_name) LOOP
    FOR c IN (SELECT column_name, data_type FROM dba_tab_cols
              WHERE owner = 'ODR' AND table_name = t.table_name AND hidden_column = 'NO'
              ORDER BY column_id) LOOP
      v_sql := 'SELECT ''' || t.table_name || '|'' || ID || ''|' || c.column_name || '|'' || ROWIDTOCHAR(ROWID) || ''|'' || ' ||
               'NVL(DUMP("' || c.column_name || '",1016),''NULL'') || ''|'' || ' ||
               CASE
                 WHEN c.data_type LIKE 'RAW%' THEN 'RAWTOHEX("' || c.column_name || '")'
                 WHEN c.data_type LIKE '%CHAR%' THEN 'ASCIISTR("' || c.column_name || '")'
                 WHEN c.data_type IN ('BINARY_FLOAT','BINARY_DOUBLE') THEN 'TO_CHAR("' || c.column_name || '",''TM9'')'
                 ELSE 'TO_CHAR("' || c.column_name || '")'
               END ||
               ' FROM ODR."' || t.table_name || '" ORDER BY ID';
      OPEN v_cur FOR v_sql;
      LOOP
        FETCH v_cur INTO v_line;
        EXIT WHEN v_cur%NOTFOUND;
        DBMS_OUTPUT.PUT_LINE(v_line);
      END LOOP;
      CLOSE v_cur;
    END LOOP;
  END LOOP;
END;
/
SPOOL OFF

SPOOL &2/meta/rowcounts.txt
DECLARE n NUMBER;
BEGIN
  FOR t IN (SELECT table_name FROM dba_tables WHERE owner = 'ODR' ORDER BY table_name) LOOP
    EXECUTE IMMEDIATE 'SELECT COUNT(*) FROM ODR."' || t.table_name || '"' INTO n;
    DBMS_OUTPUT.PUT_LINE(t.table_name || '|' || n);
  END LOOP;
END;
/
SPOOL OFF

-- Physical location of every row (file, block, slot) — lets tests compare ODR's
-- provenance with Oracle's own ROWID decoding.
SPOOL &2/meta/rowids.psv
DECLARE
  v_cur SYS_REFCURSOR; v_line VARCHAR2(4000);
BEGIN
  FOR t IN (SELECT table_name FROM dba_tables WHERE owner = 'ODR' ORDER BY table_name) LOOP
    OPEN v_cur FOR 'SELECT ''' || t.table_name || '|'' || ID || ''|'' || DBMS_ROWID.ROWID_OBJECT(ROWID) || ''|'' || ' ||
                   'DBMS_ROWID.ROWID_RELATIVE_FNO(ROWID) || ''|'' || DBMS_ROWID.ROWID_BLOCK_NUMBER(ROWID) || ''|'' || ' ||
                   'DBMS_ROWID.ROWID_ROW_NUMBER(ROWID) FROM ODR."' || t.table_name || '" ORDER BY ID';
    LOOP FETCH v_cur INTO v_line; EXIT WHEN v_cur%NOTFOUND; DBMS_OUTPUT.PUT_LINE(v_line); END LOOP;
    CLOSE v_cur;
  END LOOP;
END;
/
SPOOL OFF
EXIT
