import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { rpc, type EnvVar, type EnvVarScope, type RpcRequest } from "@/lib/api";

export function EnvVarsPage() {
  const [scope, setScope] = useState<EnvVarScope>("user");
  const [vars, setVars] = useState<EnvVar[]>([]);
  const [newName, setNewName] = useState("");
  const [newValue, setNewValue] = useState("");
  const [editing, setEditing] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);

  const load = async (s: EnvVarScope) => {
    const res = await rpc<{ type: "EnvVars"; data: EnvVar[] }>({
      method: "ListEnvVars",
      params: { scope: s },
    });
    setVars(res.data);
  };

  useEffect(() => {
    setEditing(null);
    load(scope).catch((e) => setError(e instanceof Error ? e.message : String(e)));
  }, [scope]);

  // Returns true on success so callers only clear inputs after a write landed.
  const run = async (req: RpcRequest): Promise<boolean> => {
    setError(null);
    try {
      await rpc(req);
      await load(scope);
      return true;
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      return false;
    }
  };

  const add = async () => {
    const name = newName.trim();
    if (!name) return;
    if (await run({ method: "SetEnvVar", params: { scope, name, value: newValue } })) {
      setNewName("");
      setNewValue("");
    }
  };

  const saveEdit = async (name: string) => {
    if (await run({ method: "SetEnvVar", params: { scope, name, value: draft } })) {
      setEditing(null);
    }
  };

  const remove = (name: string) => {
    if (!window.confirm(`Delete ${name} from the ${scope} environment?`)) return;
    void run({ method: "DeleteEnvVar", params: { scope, name } });
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle>Environment variables</CardTitle>
        <CardDescription>
          Windows {scope === "user" ? "user" : "system"} scope. Changes apply to newly started
          processes. System scope needs administrator rights.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="flex gap-2">
          {(["user", "system"] as const).map((s) => (
            <Button
              key={s}
              type="button"
              variant={scope === s ? "default" : "outline"}
              onClick={() => setScope(s)}
            >
              {s === "user" ? "User" : "System"}
            </Button>
          ))}
        </div>
        {error && (
          <div className="rounded-md border border-[var(--color-destructive)]/50 px-3 py-2 text-sm">
            {error}
          </div>
        )}
        <div className="flex gap-2">
          <Input
            placeholder="NAME"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
          />
          <Input
            placeholder="value"
            value={newValue}
            onChange={(e) => setNewValue(e.target.value)}
          />
          <Button type="button" variant="outline" onClick={() => void add()}>
            Add
          </Button>
        </div>
        <div className="space-y-2">
          {vars.length === 0 && (
            <span className="text-xs text-[var(--color-muted-foreground)]">None</span>
          )}
          {vars.map((v) =>
            editing === v.name ? (
              <div key={v.name} className="flex gap-2">
                <span className="w-48 truncate pt-2 font-mono text-sm">{v.name}</span>
                <Input value={draft} onChange={(e) => setDraft(e.target.value)} />
                <Button type="button" onClick={() => void saveEdit(v.name)}>
                  Save
                </Button>
                <Button type="button" variant="ghost" onClick={() => setEditing(null)}>
                  Cancel
                </Button>
              </div>
            ) : (
              <div key={v.name} className="flex items-center gap-2">
                <span className="w-48 truncate font-mono text-sm">{v.name}</span>
                <span className="min-w-0 flex-1 truncate text-sm text-[var(--color-muted-foreground)]">
                  {v.value}
                </span>
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => {
                    setEditing(v.name);
                    setDraft(v.value);
                  }}
                >
                  Edit
                </Button>
                <Button type="button" variant="destructive" onClick={() => remove(v.name)}>
                  Delete
                </Button>
              </div>
            ),
          )}
        </div>
      </CardContent>
    </Card>
  );
}
