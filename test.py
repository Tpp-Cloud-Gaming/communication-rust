import socket

def send_string_to_port(host, port, message):
    try:
        # Create a socket object
        client_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)

        # Connect to the server (replace 'localhost' with the actual server address)
        client_socket.connect((host, port))

        # Send the message
        client_socket.sendall(message.encode())

        # Close the connection
        client_socket.close()
        print(f"Message '{message}' sent successfully to {host}:{port}")
    except Exception as e:
        print(f"Error sending message: {e}")

# Example usage
if __name__ == "__main__":
    server_host = "localhost"  # Replace with the actual server address
    server_port = 2930  # Replace with the desired port number
    message_to_send = "startGameWithUser|axel|franco_god|Cuphead\n"
    #message_to_send = "startOffering|franco\n"

    send_string_to_port(server_host, server_port, message_to_send)