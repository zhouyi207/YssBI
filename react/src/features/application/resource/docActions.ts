import { DocService } from "@/services/doc/docService";
import { createFileActions } from "./createFileActions";
export const docActions = createFileActions("doc", DocService);
