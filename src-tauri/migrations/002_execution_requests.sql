CREATE TABLE execution_requests (
    requestId TEXT PRIMARY KEY REFERENCES runs(requestId),
    tokenHash TEXT NOT NULL REFERENCES confirmations(tokenHash)
);
