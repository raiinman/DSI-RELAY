using { /Fortnite.com/Devices }
using { /Verse.org/Simulation }
using { /Verse.org/Verse }
using { /UnrealEngine.com/Temporary/Diagnostics }

relay_telemetry_channel := class(log_channel){}

# Place one instance of this device in a UEFN island. It records only its own
# lifecycle and a device-ready probe; it does not inspect or alter gameplay.
relay_telemetry_example := class(creative_device):
    Logger:log = log{Channel := relay_telemetry_channel}

    # Set a different number for each captured play session. Numeric input keeps
    # the interpolated session ID a safe RELAY token.
    @editable
    SessionNumber:int = 1

    var SessionStartSeconds:float = 0.0

    SessionTimeMs():int =
        if (Milliseconds := Floor[(GetSimulationElapsedTime() - SessionStartSeconds) * 1000.0]):
            Milliseconds
        else:
            0

    OnBegin<override>()<suspends>:void=
        set SessionStartSeconds = GetSimulationElapsedTime()
        Logger.Print("RELAY_EVENT_V1 \{\"schema_version\":1,\"session_id\":\"example_{SessionNumber}\",\"sequence\":1,\"session_time_ms\":0,\"kind\":\"session_start\"\}")
        Logger.Print("RELAY_EVENT_V1 \{\"schema_version\":1,\"session_id\":\"example_{SessionNumber}\",\"sequence\":2,\"session_time_ms\":{SessionTimeMs()},\"kind\":\"project_device_ready\",\"subject_ref\":\"relay_telemetry_example\",\"state\":\"observed\"\}")

    OnEnd<override>():void=
        Logger.Print("RELAY_EVENT_V1 \{\"schema_version\":1,\"session_id\":\"example_{SessionNumber}\",\"sequence\":3,\"session_time_ms\":{SessionTimeMs()},\"kind\":\"session_end\"\}")
