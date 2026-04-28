import { type FormEvent, useState } from "react";
import { VscDatabase, VscClose } from "react-icons/vsc";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { SqlConnectionDialogOptions } from "@/shared/types/ui";

const DEFAULT_PORTS = { postgres: 5432, mysql: 3306, mariadb: 3306 } as const;

function buildConnectionString(
    engine: "postgres" | "mysql" | "mariadb",
    host: string,
    port: number,
    user: string,
    password: string,
    database: string
): string {
    const proto = engine === "postgres" ? "postgres" : "mysql";
    const enc = encodeURIComponent;
    const auth = password ? `${enc(user)}:${enc(password)}` : enc(user);
    return `${proto}://${auth}@${host}:${port}/${enc(database)}`;
}

export const SqlConnectionModal = ({
    options,
    onClose,
}: {
    options: SqlConnectionDialogOptions;
    onClose: () => void;
}) => {
    const { engine, onConnect } = options;
    const [host, setHost] = useState("localhost");
    const [port, setPort] = useState(String(DEFAULT_PORTS[engine]));
    const [user, setUser] = useState("");
    const [password, setPassword] = useState("");
    const [database, setDatabase] = useState("");
    const [rawUrl, setRawUrl] = useState("");
    const [useRaw, setUseRaw] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const label = engine === "postgres" ? "PostgreSQL" : engine === "mysql" ? "MySQL" : "MariaDB";

    const handleSubmit = (event?: FormEvent) => {
        event?.preventDefault();
        setError(null);
        try {
            const connStr = useRaw
                ? rawUrl.trim()
                : buildConnectionString(
                      engine,
                      host.trim() || "localhost",
                      parseInt(port, 10) || DEFAULT_PORTS[engine],
                      user.trim(),
                      password,
                      database.trim()
                  );
            if (!connStr) {
                setError("请填写连接信息");
                return;
            }
            onConnect(connStr);
            onClose();
        } catch (e) {
            setError(String(e));
        }
    };

    return (
        <Dialog open onOpenChange={(open) => !open && onClose()}>
            <DialogContent className="max-w-[460px]">
                <form onSubmit={handleSubmit}>
                <DialogHeader className="border-b border-border bg-muted/20">
                    <div className="flex items-center justify-between gap-4">
                        <DialogTitle className="flex items-center gap-2">
                            <VscDatabase className="text-blue-400" size={18} /> 连接 {label}
                        </DialogTitle>
                        <Button type="button" variant="ghost" size="icon-sm" onClick={onClose} aria-label="关闭">
                        <VscClose size={20} />
                        </Button>
                    </div>
                </DialogHeader>

                <div className="p-6 space-y-4">
                    <div className="grid grid-cols-2 gap-2 rounded-lg border border-border bg-muted/20 p-1">
                        <Button type="button" variant={!useRaw ? "secondary" : "ghost"} size="sm" onClick={() => setUseRaw(false)}>
                            表单配置
                        </Button>
                        <Button type="button" variant={useRaw ? "secondary" : "ghost"} size="sm" onClick={() => setUseRaw(true)}>
                            连接字符串
                        </Button>
                    </div>

                    {useRaw ? (
                        <div>
                            <label className="block text-[11px] text-muted-foreground mb-1">连接字符串</label>
                            <Input
                                type="text"
                                value={rawUrl}
                                onChange={(e) => setRawUrl(e.target.value)}
                                placeholder={
                                    engine === "postgres"
                                        ? "postgres://user:password@host:5432/database"
                                        : "mysql://user:password@host:3306/database"
                                }
                            />
                        </div>
                    ) : (
                        <>
                            <div className="grid grid-cols-2 gap-3">
                                <div>
                                    <label className="block text-[11px] text-muted-foreground mb-1">主机</label>
                                    <Input
                                        type="text"
                                        value={host}
                                        onChange={(e) => setHost(e.target.value)}
                                        placeholder="localhost"
                                    />
                                </div>
                                <div>
                                    <label className="block text-[11px] text-muted-foreground mb-1">端口</label>
                                    <Input
                                        type="number"
                                        value={port}
                                        onChange={(e) => setPort(e.target.value)}
                                    />
                                </div>
                            </div>
                            <div>
                                <label className="block text-[11px] text-muted-foreground mb-1">用户名</label>
                                <Input
                                    type="text"
                                    value={user}
                                    onChange={(e) => setUser(e.target.value)}
                                />
                            </div>
                            <div>
                                <label className="block text-[11px] text-muted-foreground mb-1">密码</label>
                                <Input
                                    type="password"
                                    value={password}
                                    onChange={(e) => setPassword(e.target.value)}
                                />
                            </div>
                            <div>
                                <label className="block text-[11px] text-muted-foreground mb-1">数据库</label>
                                <Input
                                    type="text"
                                    value={database}
                                    onChange={(e) => setDatabase(e.target.value)}
                                />
                            </div>
                        </>
                    )}

                    {error && <Badge variant="destructive">{error}</Badge>}
                </div>

                <DialogFooter>
                    <Button type="button" onClick={onClose} variant="ghost" size="lg">
                        取消
                    </Button>
                    <Button type="submit" size="lg">
                        连接
                    </Button>
                </DialogFooter>
                </form>
            </DialogContent>
        </Dialog>
    );
};
