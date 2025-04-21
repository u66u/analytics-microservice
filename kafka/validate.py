import json
import time
import random
from kafka.admin import KafkaAdminClient, NewTopic
from kafka.errors import TopicAlreadyExistsError, NoBrokersAvailable, KafkaError
from kafka.producer import KafkaProducer
from kafka.consumer import KafkaConsumer

BROKER_LIST = ["localhost:29092", "localhost:39092", "localhost:49092"]
TOPIC_NAME = "user-events"
NUM_PARTITIONS = 6
REPLICATION_FACTOR = 3
TEST_CONSUMER_GROUP = "validation-script-group"
CONNECTION_TIMEOUT_S = 10
MAX_TOPIC_CREATE_ATTEMPTS = 5
RETRY_DELAY_SECONDS = 5


def create_kafka_topic_with_retry(admin_client, topic_name, partitions, replication):
    topic = NewTopic(
        name=topic_name, num_partitions=partitions, replication_factor=replication
    )
    attempts = 0
    while attempts < MAX_TOPIC_CREATE_ATTEMPTS:
        attempts += 1
        print(
            f"Attempting to create topic '{topic_name}' (Attempt {attempts}/{MAX_TOPIC_CREATE_ATTEMPTS})..."
        )
        try:
            futures = admin_client.create_topics(
                new_topics=[topic], validate_only=False
            )
            for topic_future_name, future in futures.items():
                try:
                    future.get(timeout=15)
                    print(
                        f"Topic '{topic_future_name}' creation request processed successfully (or topic exists)."
                    )
                except TopicAlreadyExistsError:
                    print(f"Topic '{topic_future_name}' already exists.")
                except Exception as e:
                    print(
                        f"Error waiting for future result for topic '{topic_future_name}' on attempt {attempts}: {e}"
                    )
                    if isinstance(e, (KafkaError, AttributeError)):
                        print(
                            f"Retryable error encountered, waiting {RETRY_DELAY_SECONDS}s..."
                        )
                        if attempts < MAX_TOPIC_CREATE_ATTEMPTS:
                            time.sleep(RETRY_DELAY_SECONDS)
                            continue
                        else:
                            raise
                    else:
                        raise

            return True

        except TopicAlreadyExistsError:
            print(f"Topic '{topic_name}' already exists.")
            return True
        except AttributeError as ae:
            print(
                f"AttributeError during topic creation attempt {attempts}: {ae}. Likely transient state."
            )
            if attempts < MAX_TOPIC_CREATE_ATTEMPTS:
                print(f"Waiting {RETRY_DELAY_SECONDS}s before retrying...")
                time.sleep(RETRY_DELAY_SECONDS)
            else:
                print("Max retries reached after AttributeError.")
                return False
        except KafkaError as ke:
            print(
                f"KafkaError during topic creation attempt {attempts}: {ke}. Might be transient."
            )
            if attempts < MAX_TOPIC_CREATE_ATTEMPTS:
                print(f"Waiting {RETRY_DELAY_SECONDS}s before retrying...")
                time.sleep(RETRY_DELAY_SECONDS)
            else:
                print("Max retries reached after KafkaError.")
                return False
        except Exception as e:
            print(f"Non-retryable error during topic creation attempt {attempts}: {e}")
            return False

    print(
        f"Failed to create topic '{topic_name}' after {MAX_TOPIC_CREATE_ATTEMPTS} attempts."
    )
    return False


def produce_message(producer, topic_name):
    user_id = random.randint(1000, 9999)
    message = {
        "user_id": user_id,
        "event_type": "test_event",
        "action": "validation",
        "info": {"timestamp": time.time(), "source": "validation_script"},
    }
    try:
        print(f"Producing message to topic '{topic_name}': {message}")
        future = producer.send(topic_name, value=message)
        record_metadata = future.get(timeout=10)
        print(
            f"Message produced successfully to partition {record_metadata.partition} at offset {record_metadata.offset}"
        )
        return message
    except Exception as e:
        print(f"Failed to produce message: {e}")
        return None


def consume_message(consumer, expected_message):
    print(
        f"Attempting to consume message from topic '{TOPIC_NAME}' with group '{TEST_CONSUMER_GROUP}'..."
    )
    try:
        for message in consumer:
            print(
                f"Consumed message: Partition={message.partition}, Offset={message.offset}, Value={message.value}"
            )
            if message.value and message.value.get("action") == "validation":
                if message.value.get("user_id") == expected_message.get("user_id"):
                    print(
                        "\n*** Success! Consumed the message produced by this script. ***"
                    )
                    return True
                else:
                    print(
                        "Consumed a validation message, but not the one sent in this run. Continuing..."
                    )
    except Exception as e:
        print(f"An error occurred during consumption: {e}")
    except KeyboardInterrupt:
        print("Consumption interrupted.")

    print("\n*** Warning: Did not consume the expected message within the timeout. ***")
    print(
        "Possible reasons: Producer failed, consumer group started from a later offset, network issue."
    )
    return False


if __name__ == "__main__":
    admin = None
    producer = None
    consumer = None
    success = False
    sent_message = None

    print("Starting Kafka Cluster Validation...")

    initial_wait_seconds = 5
    print(
        f"Waiting {initial_wait_seconds} seconds for initial Kafka connection possibility..."
    )
    time.sleep(initial_wait_seconds)

    try:
        print(f"Connecting AdminClient to brokers: {BROKER_LIST}")
        admin = KafkaAdminClient(
            bootstrap_servers=BROKER_LIST,
            client_id="validation-admin-client",
            request_timeout_ms=20000,
        )
        print("AdminClient connected (or connection pending).")

        if not create_kafka_topic_with_retry(
            admin, TOPIC_NAME, NUM_PARTITIONS, REPLICATION_FACTOR
        ):
            raise ConnectionError("Failed to ensure topic exists after retries.")

    except NoBrokersAvailable:
        print(
            f"FATAL: Could not connect AdminClient to any brokers at {BROKER_LIST}. Is the cluster running?"
        )
        exit(1)
    except Exception as e:
        print(f"FATAL: Error during admin client connection or topic creation: {e}")
        exit(1)
    finally:
        if admin:
            admin.close()

    try:
        print(f"Connecting Producer to brokers: {BROKER_LIST}")
        producer = KafkaProducer(
            bootstrap_servers=BROKER_LIST,
            value_serializer=lambda v: json.dumps(v).encode("utf-8"),
            acks="all",
            retries=3,
        )
        sent_message = produce_message(producer, TOPIC_NAME)
        if not sent_message:
            raise ConnectionError("Failed to produce message.")

    except NoBrokersAvailable:
        print(f"FATAL: Could not connect Producer to any brokers at {BROKER_LIST}.")
        exit(1)
    except Exception as e:
        print(f"FATAL: Error during producer connection or message sending: {e}")
        exit(1)
    finally:
        if producer:
            producer.flush()
            producer.close()

    if sent_message:
        try:
            print(f"Connecting Consumer to brokers: {BROKER_LIST}")
            consumer = KafkaConsumer(
                TOPIC_NAME,
                bootstrap_servers=BROKER_LIST,
                group_id=TEST_CONSUMER_GROUP,
                auto_offset_reset="earliest",
                consumer_timeout_ms=10000,
                value_deserializer=lambda v: json.loads(v.decode("utf-8")),
            )
            success = consume_message(consumer, sent_message)

        except NoBrokersAvailable:
            print(f"FATAL: Could not connect Consumer to any brokers at {BROKER_LIST}.")
            exit(1)
        except Exception as e:
            print(
                f"FATAL: Error during consumer connection or message consumption: {e}"
            )
            exit(1)
        finally:
            if consumer:
                consumer.close()

    print("\nValidation Complete.")
    if success:
        print("Cluster appears to be working correctly for basic produce/consume.")
    else:
        print("Validation encountered issues. Please check logs and configuration.")
        exit(1)
