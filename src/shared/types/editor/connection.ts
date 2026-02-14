export interface ConnectionDTO {
  connections: {
    from_pin: string;
    to_pin: string;
  }[];
}

export type Connection = ConnectionDTO;

// 前后端转换辅助函数
export const ConnectionConverter = {
  fromDTO(dto: ConnectionDTO): Connection {
    return dto;
  },

  toDTO(connection: Connection): ConnectionDTO {
    return connection;
  },
};