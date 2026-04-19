class GitTodo < Formula
  desc "Track todos as commits on a 'todo' branch"
  homepage "https://github.com/stoggi/git-todo"
  license "MIT"
  head "https://github.com/stoggi/git-todo.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
    (man1/"git-todo.1").write Utils.safe_popen_read(bin/"git-todo", "--generate-man")
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/git-todo --version")
  end
end
