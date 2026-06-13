import { notifications } from "@mantine/notifications";
import { QueryClient } from "@tanstack/react-query";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      throwOnError: false,
    },
    mutations: {
      throwOnError: false,
      onError(error, _variables, _context) {
        notifications.show({
          color: "red",
          title: "Error",
          message: error.message
        })
      },
    }
  }
})
