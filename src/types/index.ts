/** Mirrors Rust types from src-tauri/src/types/mod.rs (camelCase via serde) */

export interface RepoRef {
  owner: string;
  name: string;
}

export type PrState = "open" | "closed" | "merged";
export type ReviewStatus = "approved" | "changesRequested" | "pending" | "reviewRequired";
export type CiStatus = "success" | "failure" | "pending" | "none";
export type Mergeable = "clean" | "conflicting" | "unknown";

export interface Author {
  login: string;
  avatarUrl: string;
}

export interface Label {
  name: string;
  color: string;
}

export interface PullRequest {
  id: string;
  number: number;
  title: string;
  state: PrState;
  isDraft: boolean;
  author: Author;
  repo: RepoRef;
  accountId: string;
  baseRef: string;
  headRef: string;
  url: string;
  createdAt: string;
  updatedAt: string;
  mergeable: Mergeable;
  reviewStatus: ReviewStatus;
  reviewRequests: string[];
  ciStatus: CiStatus;
  commentCount: number;
  labels: Label[];
  lastCommentAt: string | null;
}

export interface Account {
  id: string;
  username: string;
  avatarUrl: string;
  orgs: string[];
}
