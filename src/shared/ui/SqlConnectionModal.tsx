import { type FormEvent, useState } from "react";
import { useTranslation } from "react-i18next";
import { VscDatabase, VscClose } from "react-icons/vsc";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
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
    const { t } = useTranslation();
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
                setError(t("importModal.connectionRequired"));
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
                            <VscDatabase className="text-blue-400" size={18} /> {t("importModal.connectTo", { name: label })}
                        </DialogTitle>
                        <Button type="button" variant="ghost" size="icon-sm" onClick={onClose} aria-label={t("importModal.close")}>
                        <VscClose size={20} />
                        </Button>
                    </div>
                </DialogHeader>

                <div className="p-6 space-y-4">
                    <div className="grid grid-cols-2 gap-2 rounded-lg border border-border bg-muted/20 p-1">
                        <Button type="button" variant={!useRaw ? "secondary" : "ghost"} size="sm" onClick={() => setUseRaw(false)}>
                            {t("importModal.formConfig")}
                        </Button>
                        <Button type="button" variant={useRaw ? "secondary" : "ghost"} size="sm" onClick={() => setUseRaw(true)}>
                            {t("importModal.connectionString")}
                        </Button>
                    </div>

                    {useRaw ? (
                        <div className="space-y-1.5">
                            <Label>{t("importModal.connectionString")}</Label>
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
                                <div className="space-y-1.5">
                                    <Label>{t("importModal.host")}</Label>
                                    <Input
                                        type="text"
                                        value={host}
                                        onChange={(e) => setHost(e.target.value)}
                                        placeholder="localhost"
                                    />
                                </div>
                                <div className="space-y-1.5">
                                    <Label>{t("importModal.port")}</Label>
                                    <Input
                                        type="number"
                                        value={port}
                                        onChange={(e) => setPort(e.target.value)}
                                    />
                                </div>
                            </div>
                            <div className="space-y-1.5">
                                <Label>{t("importModal.username")}</Label>
                                <Input
                                    type="text"
                                    value={user}
                                    onChange={(e) => setUser(e.target.value)}
                                />
                            </div>
                            <div className="space-y-1.5">
                                <Label>{t("importModal.password")}</Label>
                                <Input
                                    type="password"
                                    value={password}
                                    onChange={(e) => setPassword(e.target.value)}
                                />
                            </div>
                            <div className="space-y-1.5">
                                <Label>{t("importModal.database")}</Label>
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
                        {t("common.cancel")}
                    </Button>
                    <Button type="submit" size="lg">
                        {t("importModal.connect")}
                    </Button>
                </DialogFooter>
                </form>
            </DialogContent>
        </Dialog>
    );
};
