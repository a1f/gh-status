const express = require("express");
const path = require("path");
const fs = require("fs");

const app = express();
const PORT = 4000;

const VALID_TOKEN = "ghp_test_token_valid";
const RATE_LIMITED_TOKEN = "ghp_test_rate_limited";

const FIXTURES_DIR = path.join(__dirname, "fixtures");

function loadFixture(name) {
  const filePath = path.join(FIXTURES_DIR, name);
  return JSON.parse(fs.readFileSync(filePath, "utf-8"));
}

const viewerResponse = loadFixture("viewer.json");
const pullRequestsResponse = loadFixture("pull-requests.json");

app.use(express.json());

app.post("/graphql", (req, res) => {
  const authHeader = req.headers.authorization || "";
  const token = authHeader.replace("Bearer ", "");

  if (token === RATE_LIMITED_TOKEN) {
    return res
      .status(403)
      .set({
        "X-RateLimit-Limit": "5000",
        "X-RateLimit-Remaining": "0",
        "X-RateLimit-Reset": String(Math.floor(Date.now() / 1000) + 3600),
      })
      .json({
        message: "API rate limit exceeded",
        documentation_url: "https://docs.github.com/rest/overview/resources-in-the-rest-api#rate-limiting",
      });
  }

  if (token !== VALID_TOKEN) {
    return res.status(401).json({ message: "Bad credentials" });
  }

  const query = (req.body && req.body.query) || "";

  res.set({
    "X-RateLimit-Limit": "5000",
    "X-RateLimit-Remaining": "4999",
  });

  if (query.includes("viewer")) {
    return res.json(viewerResponse);
  }

  if (query.includes("pullRequests")) {
    return res.json(pullRequestsResponse);
  }

  res.status(400).json({
    errors: [{ message: "Unrecognized query" }],
  });
});

app.get("/health", (_req, res) => {
  res.json({ status: "ok" });
});

app.listen(PORT, () => {
  console.log(`Mock GitHub GraphQL API listening on port ${PORT}`);
});
