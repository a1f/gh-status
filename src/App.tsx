function App() {
  return (
    <div style={{ display: "flex", height: "100vh" }}>
      <aside style={{ width: 260, borderRight: "1px solid #333", padding: 16 }}>
        <h2>gh-status</h2>
        <p style={{ color: "#888", fontSize: 13 }}>PR Dashboard</p>
      </aside>
      <main style={{ flex: 1, padding: 16 }}>
        <h1>Welcome to gh-status</h1>
        <p>Your GitHub PR dashboard is being built.</p>
      </main>
    </div>
  );
}

export default App;
