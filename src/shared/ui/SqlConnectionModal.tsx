import { useState } from "react";
import { VscDatabase, VscClose } from "react-icons/vsc";
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

    const handleSubmit = () => {
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
        <div className="fixed inset-0 z-[500] flex items-center justify-center bg-black/60 backdrop-blur-sm animate-fade-in">
            <div className="bg-[#1e1e1e] border border-gray-700 rounded-xl shadow-2xl w-[420px] overflow-hidden animate-zoom-in">
                <div className="px-6 py-4 border-b border-gray-800 bg-[#252526] flex justify-between items-center">
                    <h3 className="text-sm font-bold text-white flex items-center gap-2 uppercase tracking-wider">
                        <VscDatabase className="text-blue-500" size={18} /> 连接 {label}
                    </h3>
                    <button onClick={onClose} className="text-gray-500 hover:text-white transition-colors">
                        <VscClose size={20} />
                    </button>
                </div>

                <div className="p-6 space-y-4">
                    <label className="flex items-center gap-2 text-xs text-gray-400 cursor-pointer">
                        <input
                            type="checkbox"
                            checked={useRaw}
                            onChange={(e) => setUseRaw(e.target.checked)}
                            className="rounded"
                        />
                        使用连接字符串
                    </label>

                    {useRaw ? (
                        <div>
                            <label className="block text-[11px] text-gray-500 mb-1">连接字符串</label>
                            <input
                                type="text"
                                value={rawUrl}
                                onChange={(e) => setRawUrl(e.target.value)}
                                placeholder={
                                    engine === "postgres"
                                        ? "postgres://user:password@host:5432/database"
                                        : "mysql://user:password@host:3306/database"
                                }
                                className="w-full px-3 py-2 rounded-lg bg-gray-800 border border-gray-700 text-white text-sm placeholder-gray-500 focus:border-[var(--accent-color)] focus:outline-none"
                            />
                        </div>
                    ) : (
                        <>
                            <div className="grid grid-cols-2 gap-3">
                                <div>
                                    <label className="block text-[11px] text-gray-500 mb-1">主机</label>
                                    <input
                                        type="text"
                                        value={host}
                                        onChange={(e) => setHost(e.target.value)}
                                        placeholder="localhost"
                                        className="w-full px-3 py-2 rounded-lg bg-gray-800 border border-gray-700 text-white text-sm placeholder-gray-500 focus:border-[var(--accent-color)] focus:outline-none"
                                    />
                                </div>
                                <div>
                                    <label className="block text-[11px] text-gray-500 mb-1">端口</label>
                                    <input
                                        type="number"
                                        value={port}
                                        onChange={(e) => setPort(e.target.value)}
                                        className="w-full px-3 py-2 rounded-lg bg-gray-800 border border-gray-700 text-white text-sm focus:border-[var(--accent-color)] focus:outline-none"
                                    />
                                </div>
                            </div>
                            <div>
                                <label className="block text-[11px] text-gray-500 mb-1">用户名</label>
                                <input
                                    type="text"
                                    value={user}
                                    onChange={(e) => setUser(e.target.value)}
                                    className="w-full px-3 py-2 rounded-lg bg-gray-800 border border-gray-700 text-white text-sm placeholder-gray-500 focus:border-[var(--accent-color)] focus:outline-none"
                                />
                            </div>
                            <div>
                                <label className="block text-[11px] text-gray-500 mb-1">密码</label>
                                <input
                                    type="password"
                                    value={password}
                                    onChange={(e) => setPassword(e.target.value)}
                                    className="w-full px-3 py-2 rounded-lg bg-gray-800 border border-gray-700 text-white text-sm placeholder-gray-500 focus:border-[var(--accent-color)] focus:outline-none"
                                />
                            </div>
                            <div>
                                <label className="block text-[11px] text-gray-500 mb-1">数据库</label>
                                <input
                                    type="text"
                                    value={database}
                                    onChange={(e) => setDatabase(e.target.value)}
                                    className="w-full px-3 py-2 rounded-lg bg-gray-800 border border-gray-700 text-white text-sm placeholder-gray-500 focus:border-[var(--accent-color)] focus:outline-none"
                                />
                            </div>
                        </>
                    )}

                    {error && <p className="text-xs text-red-400">{error}</p>}
                </div>

                <div className="px-6 py-4 bg-[#252526] border-t border-gray-800 flex justify-end gap-2">
                    <button
                        onClick={onClose}
                        className="px-4 py-2 rounded-lg text-sm text-gray-400 hover:text-white transition-colors"
                    >
                        取消
                    </button>
                    <button
                        onClick={handleSubmit}
                        className="px-4 py-2 rounded-lg text-sm font-medium bg-[var(--accent-color)] text-white hover:opacity-90 transition-opacity"
                    >
                        连接
                    </button>
                </div>
            </div>
        </div>
    );
};
