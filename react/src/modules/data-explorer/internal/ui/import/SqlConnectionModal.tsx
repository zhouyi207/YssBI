import { type FormEvent, useRef, useState } from "react";
import { ui } from "@/features/core/ui/ui";
import { useTranslation } from "react-i18next";
import { VscDatabase, VscClose } from "react-icons/vsc";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { SqlConnectionDialogOptions } from "@/features/application/ui/applicationUi";

import { useImportStep } from "./useImportStep";

const DEFAULT_PORTS = { postgres: 5432, mysql: 3306, mariadb: 3306 } as const;

function buildConnectionString(
  engine: "postgres" | "mysql" | "mariadb",
  host: string,
  port: number,
  user: string,
  password: string,
  database: string,
): string {
  const proto = engine === "postgres" ? "postgres" : "mysql";
  const enc = encodeURIComponent;
  const auth = password ? `${enc(user)}:${enc(password)}` : enc(user);
  return `${proto}://${auth}@${host}:${port}/${enc(database)}`;
}

export const SqlConnectionModal = ({
  modalId,
  options,
  onClose,
}: {
  modalId: string;
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
  const { busy, error, setError, run } = useImportStep(onConnect);
  const confirmingClose = useRef(false);
  const requestClose = async () => {
    if (busy || confirmingClose.current) return;
    const dirty =
      host !== "localhost" ||
      port !== String(DEFAULT_PORTS[engine]) ||
      user !== "" ||
      password !== "" ||
      database !== "" ||
      rawUrl !== "";
    if (!dirty) {
      onClose();
      return;
    }
    confirmingClose.current = true;
    try {
      if (
        await ui.confirm(
          {
            title: t("importModal.discardConnectionTitle"),
            message: t("importModal.discardConnectionMessage"),
            confirmText: t("importModal.discardConnectionConfirm"),
          },
          modalId,
        )
      )
        onClose();
    } finally {
      confirmingClose.current = false;
    }
  };

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
            database.trim(),
          );
      if (!connStr) {
        setError(t("importModal.connectionRequired"));
        return;
      }
      void run(connStr);
    } catch {
      setError(t("importModal.connectionFieldsInvalid"));
    }
  };

  return (
    <Dialog open onOpenChange={(open) => !open && void requestClose()}>
      <DialogContent className="max-w-[460px]">
        <form onSubmit={handleSubmit}>
          <DialogHeader className="border-b border-border bg-muted/20">
            <div className="flex items-center justify-between gap-4">
              <DialogTitle className="flex items-center gap-2">
                <VscDatabase className="text-blue-400" size={18} />{" "}
                {t("importModal.connectTo", { name: label })}
              </DialogTitle>
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                onClick={() => void requestClose()}
                disabled={busy}
                aria-label={t("importModal.close")}
              >
                <VscClose size={20} />
              </Button>
            </div>
          </DialogHeader>

          <fieldset disabled={busy} className="p-6 space-y-4">
            <div className="grid grid-cols-2 gap-2 rounded-lg border border-border bg-muted/20 p-1">
              <Button
                type="button"
                variant={!useRaw ? "secondary" : "ghost"}
                size="sm"
                onClick={() => setUseRaw(false)}
              >
                {t("importModal.formConfig")}
              </Button>
              <Button
                type="button"
                variant={useRaw ? "secondary" : "ghost"}
                size="sm"
                onClick={() => setUseRaw(true)}
              >
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
                    <Input type="number" value={port} onChange={(e) => setPort(e.target.value)} />
                  </div>
                </div>
                <div className="space-y-1.5">
                  <Label>{t("importModal.username")}</Label>
                  <Input type="text" value={user} onChange={(e) => setUser(e.target.value)} />
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
          </fieldset>

          <DialogFooter>
            <Button
              type="button"
              onClick={() => void requestClose()}
              disabled={busy}
              variant="ghost"
              size="lg"
            >
              {t("common.cancel")}
            </Button>
            <Button type="submit" size="lg" disabled={busy}>
              {t(busy ? "dataOperation.reading" : "importModal.connect")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
};
