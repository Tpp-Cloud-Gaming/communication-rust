import os
import glob
import pandas as pd
import matplotlib.pyplot as plt
import matplotlib.ticker as ticker

# Obtener el último archivo creado en la carpeta 'data'
list_of_files = glob.glob('src/webrtcommunication/data/*') 
latest_file = max(list_of_files, key=os.path.getctime)

# Leer los datos del archivo
data = pd.read_csv(latest_file, names=['time', 'latency'], parse_dates=['time'])

# Convertir la columna 'time' a datetime
data['time'] = pd.to_datetime(data['time'])

# Convertir la columna 'time' a una duración en segundos desde el inicio del conjunto de datos
data['time'] = (data['time'] - data['time'].iloc[0]).dt.total_seconds()

# Establecer la columna 'time' como el índice
data.set_index('time', inplace=True)

# Crear la gráfica
fig, ax = plt.subplots(figsize=(10,6))
ax.plot(data.index, data['latency'], label='Latency', color='skyblue')

# Formatear las etiquetas del eje x para mostrar minutos, segundos y milisegundos
formatter = ticker.FuncFormatter(lambda x, pos: f'{int(x // 60)}:{int(x % 60):02}.{int((x % 1) * 1000):03}')
ax.xaxis.set_major_formatter(formatter)

plt.title('Latency over time')
plt.xlabel('Time (mm:ss.ms)')
plt.ylabel('Latency (ms)')
plt.legend()

plt.show()