import { DocService } from "@/services/doc/docService";
import { docProjection } from "@/features/core/resource/docProjectionStore";
import { createFileActions } from "./createFileActions";
export const docActions = createFileActions(docProjection, DocService);
