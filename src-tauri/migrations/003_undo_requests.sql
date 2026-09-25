-- 撤销请求的完整幂等绑定与最终报告。
CREATE TABLE undo_requests (
    requestId      TEXT PRIMARY KEY REFERENCES runs(requestId) ON DELETE CASCADE,
    undoPlanId     TEXT NOT NULL REFERENCES undo_plans(id),
    originalRunId  TEXT NOT NULL REFERENCES runs(id),
    tokenHash      TEXT NOT NULL,
    digest         TEXT NOT NULL,
    selectedJson   TEXT NOT NULL,
    reportJson     TEXT
);

CREATE INDEX idx_undo_requests_original_run ON undo_requests(originalRunId);
