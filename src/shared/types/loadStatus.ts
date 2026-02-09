/**
 *  
 *  | 状态  | isLoading | isLoaded | error |
 *  | --- | --------- | -------- | ----- |
 *  | Idel  | false     | false    | null  |
 *  | Loading/Refreshing | true      | false    | null  |
 *  | Ready  | false     | true     | null  |
 *  | Error  | false     | false    | Error |
 */
export enum LoadStatus {
    Idle,
    Loading,
    Ready,
    Refreshing,
    Error,
}
